//! PNG decoding for sprites and textures (DR-38).
//!
//! The file is untrusted, like a GLB: the header is checked against limits before any pixel
//! buffer exists, the output is always 8-bit RGBA with the top row first, and nothing else in
//! the file (colour profiles, text chunks, extra APNG frames) is used.

use crate::ImportError;

/// Largest PNG the caller may hand over.
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
/// Largest side of an image, in pixels.
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// Largest number of pixels in one image (a 4096 by 2048 picture fits).
pub const MAX_IMAGE_PIXELS: usize = 8 * 1024 * 1024;

/// A decoded image: row-major RGBA, 8 bits per channel, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ImportedImage {
    /// Bytes of one row of `rgba`.
    pub fn row_bytes(&self) -> usize {
        self.width as usize * 4
    }
}

/// Reads a PNG into 8-bit RGBA. Palette, greyscale, 16-bit and transparent images are all
/// converted; a file that breaks a limit is refused before its pixels are allocated.
pub fn decode_png(bytes: &[u8]) -> Result<ImportedImage, ImportError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ImportError::Refused(format!(
            "a imagem tem mais de {} MB",
            MAX_IMAGE_BYTES / (1024 * 1024)
        )));
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // 8-bit RGBA out, whatever came in (palette, 16-bit, greyscale).
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| ImportError::Invalid(format!("a imagem não é um PNG válido: {error}")))?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if width == 0 || height == 0 {
        return Err(ImportError::Invalid(
            "a imagem não tem nenhum pixel".to_string(),
        ));
    }
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err(ImportError::Refused(format!(
            "a imagem tem {width} por {height} pixels; o limite é {MAX_IMAGE_SIDE} por lado"
        )));
    }
    let pixels = width as usize * height as usize;
    if pixels > MAX_IMAGE_PIXELS {
        return Err(ImportError::Refused(format!(
            "a imagem tem {pixels} pixels; o limite é {MAX_IMAGE_PIXELS}"
        )));
    }
    // 8-bit RGBA out, whatever came in.
    let Some(size) = reader.output_buffer_size() else {
        return Err(ImportError::Invalid(
            "a imagem não informa o tamanho dos pixels".to_string(),
        ));
    };
    if size > MAX_IMAGE_PIXELS * 4 {
        return Err(ImportError::Refused(
            "a imagem é grande demais depois de convertida".to_string(),
        ));
    }
    let mut buffer = vec![0u8; size];
    let frame = reader.next_frame(&mut buffer).map_err(|error| {
        ImportError::Invalid(format!(
            "os pixels da imagem não puderam ser lidos: {error}"
        ))
    })?;
    let used = frame.buffer_size().min(buffer.len());
    let data = buffer.get(..used).unwrap_or_default();
    if frame.width != width || frame.height != height {
        return Err(ImportError::Invalid(
            "a imagem mudou de tamanho ao ser lida".to_string(),
        ));
    }
    let rgba = to_rgba(data, frame.color_type).ok_or_else(|| {
        ImportError::Invalid("o formato de cor da imagem não pôde ser convertido".to_string())
    })?;
    Ok(ImportedImage {
        width,
        height,
        rgba,
    })
}

/// Reads a PNG file from the disk: `None` when it is missing, empty, over the size limit or not a
/// valid PNG. Whether the path may be read at all is the caller's decision: `resolve_inside` in
/// `petunia_project` is what keeps an asset inside its own project.
pub fn read_png_file(path: &std::path::Path) -> Option<ImportedImage> {
    let length = std::fs::metadata(path).ok()?.len();
    if length == 0 || length > MAX_IMAGE_BYTES as u64 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    decode_png(&bytes).ok()
}

/// Expands the decoded samples to RGBA. `None` when the sample count does not fit the format.
fn to_rgba(data: &[u8], color_type: png::ColorType) -> Option<Vec<u8>> {
    let channels = match color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return None,
    };
    if !data.len().is_multiple_of(channels) {
        return None;
    }
    let mut rgba = Vec::with_capacity(data.len() / channels * 4);
    for pixel in data.chunks_exact(channels) {
        match pixel {
            [value] => rgba.extend_from_slice(&[*value, *value, *value, 255]),
            [value, alpha] => rgba.extend_from_slice(&[*value, *value, *value, *alpha]),
            [red, green, blue] => rgba.extend_from_slice(&[*red, *green, *blue, 255]),
            [red, green, blue, alpha] => rgba.extend_from_slice(&[*red, *green, *blue, *alpha]),
            _ => return None,
        }
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("a header");
            writer.write_image_data(data).expect("the pixels");
        }
        out
    }

    #[test]
    fn rgba_round_trips_pixel_by_pixel() {
        let pixels = [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 9, 9, 9, 255];
        let file = encode(2, 2, png::ColorType::Rgba, &pixels);
        let image = decode_png(&file).expect("a valid image");
        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.row_bytes(), 8);
        assert_eq!(image.rgba, pixels);
    }

    #[test]
    fn rgb_and_greyscale_become_opaque_rgba() {
        let rgb = encode(1, 2, png::ColorType::Rgb, &[1, 2, 3, 4, 5, 6]);
        let image = decode_png(&rgb).expect("a valid image");
        assert_eq!(image.rgba, [1, 2, 3, 255, 4, 5, 6, 255]);

        let grey = encode(2, 1, png::ColorType::Grayscale, &[7, 200]);
        let image = decode_png(&grey).expect("a valid image");
        assert_eq!(image.rgba, [7, 7, 7, 255, 200, 200, 200, 255]);
    }

    #[test]
    fn the_transparency_of_a_greyscale_alpha_image_is_kept() {
        let file = encode(2, 1, png::ColorType::GrayscaleAlpha, &[10, 255, 20, 0]);
        let image = decode_png(&file).expect("a valid image");
        assert_eq!(image.rgba, [10, 10, 10, 255, 20, 20, 20, 0]);
    }

    #[test]
    fn a_file_that_is_not_a_png_is_refused() {
        let error = decode_png(b"isto nao e um png").expect_err("must be refused");
        assert!(matches!(error, ImportError::Invalid(_)), "{error:?}");
    }

    #[test]
    fn an_image_wider_than_the_limit_is_refused_before_decoding() {
        let width = MAX_IMAGE_SIDE + 1;
        let data = vec![0u8; width as usize * 4];
        let file = encode(width, 1, png::ColorType::Rgba, &data);
        let error = decode_png(&file).expect_err("must be refused");
        assert!(matches!(error, ImportError::Refused(_)), "{error:?}");
    }

    #[test]
    fn a_file_over_the_byte_limit_is_refused() {
        let mut file = vec![0u8; 8];
        file.extend(std::iter::repeat_n(0u8, MAX_IMAGE_BYTES));
        let error = decode_png(&file).expect_err("must be refused");
        assert!(matches!(error, ImportError::Refused(_)), "{error:?}");
    }

    #[test]
    fn a_png_file_is_read_from_the_disk_and_a_missing_or_empty_one_is_not() {
        let folder =
            std::env::temp_dir().join(format!("petunia_import_image_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a folder");
        let path = folder.join("hero.png");
        std::fs::write(&path, encode(1, 1, png::ColorType::Rgba, &[1, 2, 3, 4])).expect("the file");
        let image = read_png_file(&path).expect("a valid image");
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.rgba, [1, 2, 3, 4]);
        assert!(read_png_file(&folder.join("missing.png")).is_none());
        std::fs::write(folder.join("empty.png"), b"").expect("the empty file");
        assert!(read_png_file(&folder.join("empty.png")).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn to_rgba_copies_rgba_into_four_channel_pixels() {
        let out = to_rgba(&[1, 2, 3, 4], png::ColorType::Rgba).expect("four channels");
        assert_eq!(out, [1, 2, 3, 4]);
        assert!(to_rgba(&[1, 2, 3], png::ColorType::Rgba).is_none());
        assert!(to_rgba(&[], png::ColorType::Indexed).is_none());
    }
}
