//! Textures a frame carries to the renderer (DR-38).
//!
//! Decoding lives in `petunia_import` and drawing in `petunia_render_gl`; here the pixels are
//! plain data with a small handle, so a frame stays a read-only copy that any backend can upload.
//! [`TextureLoader`] is the one rule both hosts (editor and player) share: an asset is decoded
//! once, and a path that failed is not read again on every frame.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::frame::RenderFrame;

/// Handle of a texture inside one frame. It is not saved: a scene names the asset, not the handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureId(pub u32);

/// Pixels of one texture: row-major RGBA, 8 bits per channel, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TexturePixels {
    pub id: TextureId,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl TexturePixels {
    /// Builds the pixels, or `None` when the buffer does not match the size or the size is empty.
    pub fn new(id: TextureId, width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = width as usize * height as usize * 4;
        (width > 0 && height > 0 && rgba.len() == expected).then_some(Self {
            id,
            width,
            height,
            rgba,
        })
    }
}

/// The textures the host decoded, by the asset path a scene names (for example
/// `assets/hero.png`).
#[derive(Clone, Debug, Default)]
pub struct TextureStore {
    next_id: u32,
    by_asset: BTreeMap<String, Arc<TexturePixels>>,
    by_id: BTreeMap<TextureId, Arc<TexturePixels>>,
}

impl TextureStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an image under `asset` and returns its handle. A path that is already known keeps its
    /// handle and its first pixels: empty the store to show a changed file.
    pub fn insert(
        &mut self,
        asset: &str,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Option<TextureId> {
        if let Some(existing) = self.by_asset.get(asset) {
            return Some(existing.id);
        }
        let pixels = TexturePixels::new(TextureId(self.next_id), width, height, rgba)?;
        let id = pixels.id;
        let shared = Arc::new(pixels);
        self.next_id = self.next_id.saturating_add(1);
        self.by_asset.insert(asset.to_string(), Arc::clone(&shared));
        self.by_id.insert(id, shared);
        Some(id)
    }

    /// Handle of an asset, when it is loaded.
    pub fn id_of(&self, asset: &str) -> Option<TextureId> {
        self.by_asset.get(asset).map(|pixels| pixels.id)
    }

    /// Pixels of one handle, for a backend that has to upload them.
    pub fn pixels(&self, id: TextureId) -> Option<Arc<TexturePixels>> {
        self.by_id.get(&id).cloned()
    }

    /// Pixels of one asset path.
    pub fn pixels_of(&self, asset: &str) -> Option<Arc<TexturePixels>> {
        self.by_asset.get(asset).cloned()
    }

    /// Every loaded asset and its handle, in path order (so a load loop is deterministic).
    pub fn assets(&self) -> impl Iterator<Item = (&str, TextureId)> {
        self.by_asset
            .iter()
            .map(|(asset, pixels)| (asset.as_str(), pixels.id))
    }

    pub fn len(&self) -> usize {
        self.by_asset.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_asset.is_empty()
    }

    /// Forgets every texture, so the next insert assigns the handles again.
    pub fn clear(&mut self) {
        self.next_id = 0;
        self.by_asset.clear();
        self.by_id.clear();
    }
}

/// Decodes each asset a frame's sprites name, once, and remembers which paths failed.
///
/// The host decides where the bytes come from and how large they may be: `read` returns the size
/// and the RGBA pixels of an asset, or `None` when it must not be used (missing file, path outside
/// the project, not a PNG). A failed path stays failed until [`Self::clear`], so a broken sprite
/// costs one file check instead of one per frame.
#[derive(Clone, Debug, Default)]
pub struct TextureLoader {
    store: TextureStore,
    failed: BTreeSet<String>,
}

impl TextureLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every picture and every failure (a new project, or a file fixed on disk).
    pub fn clear(&mut self) {
        self.store.clear();
        self.failed.clear();
    }

    /// How many pictures are loaded.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// How many assets could not be loaded.
    pub fn failed(&self) -> usize {
        self.failed.len()
    }

    /// The store the handles come from (a host may ask for pixels by asset).
    pub fn store(&self) -> &TextureStore {
        &self.store
    }

    /// Loads what the frame's sprites name and points every sprite at its handle. Sprites whose
    /// asset is missing or broken stay plain colored quads.
    pub fn resolve<Read>(&mut self, frame: &mut RenderFrame, mut read: Read)
    where
        Read: FnMut(&str) -> Option<(u32, u32, Vec<u8>)>,
    {
        let mut wanted: Vec<&str> = frame
            .sprites
            .iter()
            .map(|sprite| sprite.asset.as_str())
            .filter(|asset| !asset.is_empty())
            .collect();
        // A deterministic order: two runs of the same scene assign the same handles.
        wanted.sort_unstable();
        wanted.dedup();
        for asset in wanted {
            if self.store.id_of(asset).is_some() || self.failed.contains(asset) {
                continue;
            }
            match read(asset) {
                Some((width, height, rgba)) => {
                    if self.store.insert(asset, width, height, rgba).is_none() {
                        self.failed.insert(asset.to_string());
                    }
                }
                None => {
                    self.failed.insert(asset.to_string());
                }
            }
        }
        frame.resolve_textures(&self.store);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(count: usize) -> Vec<u8> {
        vec![255u8; count * 4]
    }

    #[test]
    fn a_known_asset_keeps_its_handle() {
        let mut store = TextureStore::new();
        let first = store
            .insert("assets/hero.png", 2, 2, pixels(4))
            .expect("4 pixels");
        let again = store
            .insert("assets/hero.png", 2, 2, pixels(4))
            .expect("known");
        assert_eq!(first, again);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn handles_are_assigned_in_insertion_order() {
        let mut store = TextureStore::new();
        let hero = store
            .insert("assets/hero.png", 1, 1, pixels(1))
            .expect("1 pixel");
        let tile = store
            .insert("assets/tile.png", 1, 1, pixels(1))
            .expect("1 pixel");
        assert_eq!(hero, TextureId(0));
        assert_eq!(tile, TextureId(1));
        assert_eq!(store.id_of("assets/tile.png"), Some(TextureId(1)));
        assert_eq!(store.id_of("assets/none.png"), None);
    }

    #[test]
    fn a_buffer_that_does_not_match_the_size_is_refused() {
        let mut store = TextureStore::new();
        assert_eq!(store.insert("assets/bad.png", 2, 2, pixels(3)), None);
        assert_eq!(store.insert("assets/empty.png", 0, 4, Vec::new()), None);
        assert!(store.is_empty());
    }

    #[test]
    fn pixels_can_be_fetched_by_handle_or_by_asset() {
        let mut store = TextureStore::new();
        let id = store
            .insert("assets/hero.png", 2, 1, pixels(2))
            .expect("2 pixels");
        let by_id = store.pixels(id).expect("by handle");
        let by_asset = store.pixels_of("assets/hero.png").expect("by asset");
        assert_eq!(by_id, by_asset);
        assert_eq!(by_id.width, 2);
        assert_eq!(by_id.height, 1);
        assert_eq!(by_id.rgba.len(), 8);
    }

    #[test]
    fn clearing_makes_the_next_insert_start_at_the_first_handle() {
        let mut store = TextureStore::new();
        store
            .insert("assets/hero.png", 1, 1, pixels(1))
            .expect("1 pixel");
        store.clear();
        assert!(store.is_empty());
        let id = store
            .insert("assets/hero.png", 1, 1, pixels(1))
            .expect("1 pixel");
        assert_eq!(id, TextureId(0));
    }

    fn frame_with_assets(assets: &[&str]) -> RenderFrame {
        let mut frame = RenderFrame {
            camera: crate::camera::Camera::default(),
            items: Vec::new(),
            sprites: Vec::new(),
            textures: Vec::new(),
            sculpts: Vec::new(),
            overlay: Vec::new(),
            show_grid: false,
            grid_height: 0.0,
            clear_color: petunia_math::Color::WHITE,
            lighting: crate::frame::Lighting::default(),
            seconds: 0.0,
        };
        for (index, asset) in assets.iter().enumerate() {
            frame.sprites.push(crate::frame::SpriteItem {
                entity: index as u64 + 1,
                selected: false,
                hovered: false,
                width: 1.0,
                height: 1.0,
                transform: petunia_math::Transform::default(),
                color: petunia_math::Color::WHITE,
                asset: (*asset).to_string(),
                texture: None,
                flip_x: false,
                flip_y: false,
                layer: 0,
                animation: crate::frame::SpriteAnimation::single(),
                frame: 0,
            });
        }
        frame
    }

    #[test]
    fn every_sprite_points_at_the_handle_of_its_asset() {
        let mut loader = TextureLoader::new();
        let mut frame =
            frame_with_assets(&["assets/hero.png", "assets/tile.png", "assets/hero.png"]);
        loader.resolve(&mut frame, |asset| match asset {
            "assets/hero.png" => Some((1, 1, pixels(1))),
            "assets/tile.png" => Some((2, 1, pixels(2))),
            _ => None,
        });
        assert_eq!(frame.sprites[0].texture, frame.sprites[2].texture);
        assert_ne!(frame.sprites[0].texture, frame.sprites[1].texture);
        assert_eq!(
            frame.textures.len(),
            2,
            "one upload per asset, not per sprite"
        );
        assert_eq!(loader.len(), 2);
    }

    #[test]
    fn an_asset_that_cannot_be_read_is_read_once_and_leaves_a_plain_quad() {
        let mut loader = TextureLoader::new();
        let mut frame = frame_with_assets(&["assets/missing.png", "assets/missing.png"]);
        let mut reads = 0;
        for _ in 0..3 {
            loader.resolve(&mut frame, |_| {
                reads += 1;
                None
            });
        }
        assert_eq!(reads, 1, "a failed asset is not tried again");
        assert!(frame.sprites.iter().all(|sprite| sprite.texture.is_none()));
        assert!(frame.textures.is_empty());
        assert_eq!(loader.failed(), 1);
    }

    #[test]
    fn a_sprite_without_an_asset_asks_for_nothing() {
        let mut loader = TextureLoader::new();
        let mut frame = frame_with_assets(&[""]);
        let mut reads = 0;
        loader.resolve(&mut frame, |_| {
            reads += 1;
            None
        });
        assert_eq!(reads, 0);
        assert!(frame.sprites[0].texture.is_none());
    }
}
