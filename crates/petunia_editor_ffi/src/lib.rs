//! C interface of the editor.
//!
//! The GUI (Qt, C++) is a thin shell: it sends JSON requests and draws what comes back. This
//! crate is the boundary, so it is the one place besides the GL adapter that may use `unsafe`.
//!
//! Rules of the boundary:
//! - Every returned `char*` was allocated here and must be released with
//!   [`petunia_string_free`]. Never `free()` it from C.
//! - Every function accepts a null handle or null text and answers with an error instead of
//!   crashing.
//! - A panic never crosses into C: it is caught and reported as an error response. (Production
//!   code does not panic; this is the last-resort guard the engineering guide allows at the
//!   boundary.)
//! - All calls happen on the GUI thread. The viewport functions need the GL context current.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use petunia_editor::EditorSession;
use petunia_render_gl::GlRenderer;
use serde_json::{Value as Json, json};

/// Opaque to C. Holds the session and, once the GL context exists, the renderer.
pub struct EditorHandle {
    session: EditorSession,
    renderer: Option<GlRenderer>,
}

/// Function that resolves an OpenGL function by name (Qt's `getProcAddress`).
pub type GlLoader = unsafe extern "C" fn(name: *const c_char) -> *const c_void;

fn into_c_string(text: String) -> *mut c_char {
    match CString::new(text) {
        Ok(owned) => owned.into_raw(),
        Err(_) => CString::default().into_raw(),
    }
}

fn error_response(code: &str, message: &str, action: &str) -> String {
    json!({
        "ok": false,
        "error": { "code": code, "message": message, "action": action }
    })
    .to_string()
}

fn guarded<T>(fallback: impl FnOnce(String) -> T, work: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(value) => value,
        Err(_) => fallback("falha interna inesperada no editor".to_string()),
    }
}

/// Creates an editor. Returns null only if the built-in commands could not be registered.
#[unsafe(no_mangle)]
pub extern "C" fn petunia_editor_create() -> *mut EditorHandle {
    guarded(
        |_| std::ptr::null_mut(),
        || match EditorSession::new() {
            Ok(session) => Box::into_raw(Box::new(EditorHandle {
                session,
                renderer: None,
            })),
            Err(_) => std::ptr::null_mut(),
        },
    )
}

/// Destroys an editor. Call `petunia_editor_viewport_release` first, with the GL context
/// current, so GPU objects are freed properly.
///
/// # Safety
/// `handle` must be null or a pointer from [`petunia_editor_create`] not yet destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_editor_destroy(handle: *mut EditorHandle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the caller guarantees the pointer came from `petunia_editor_create`.
    drop(unsafe { Box::from_raw(handle) });
}

/// Runs one JSON request (see `EditorSession::handle`) and returns the JSON response.
///
/// # Safety
/// `handle` must be null or a live editor; `request` must be null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_editor_call(
    handle: *mut EditorHandle,
    request: *const c_char,
) -> *mut c_char {
    // SAFETY: forwarded contract.
    let (editor, request) = unsafe { (handle.as_mut(), request.as_ref()) };
    let (Some(editor), Some(request)) = (editor, request) else {
        return into_c_string(error_response(
            "EDITOR_BAD_REQUEST",
            "o editor ou o pedido é nulo",
            "Isso é um defeito da interface; registre o problema.",
        ));
    };
    // SAFETY: the caller guarantees a NUL-terminated string.
    let text = unsafe { CStr::from_ptr(std::ptr::from_ref(request)) };
    let response = guarded(
        |message| error_response("EDITOR_INTERNAL", &message, "Registre o problema."),
        || match text.to_str().map(serde_json::from_str::<Json>) {
            Ok(Ok(parsed)) => editor.session.handle(&parsed).to_string(),
            _ => error_response(
                "EDITOR_BAD_REQUEST",
                "o pedido não é um JSON válido em UTF-8",
                "Envie um objeto JSON com o campo `op`.",
            ),
        },
    );
    into_c_string(response)
}

/// Creates the renderer on the current OpenGL 3.3 context. Returns null on success, or an error
/// message (free it with [`petunia_string_free`]).
///
/// # Safety
/// `handle` must be null or a live editor; `loader` must resolve GL functions of the current
/// context; the context must stay current for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_editor_viewport_init(
    handle: *mut EditorHandle,
    loader: Option<GlLoader>,
) -> *mut c_char {
    // SAFETY: forwarded contract.
    let (Some(editor), Some(loader)) = (unsafe { handle.as_mut() }, loader) else {
        return into_c_string("o editor ou o carregador de OpenGL é nulo".to_string());
    };
    guarded(into_c_string, || {
        // SAFETY: `loader` resolves symbols of the current context (caller's contract).
        let context =
            unsafe { glow::Context::from_loader_function_cstr(|name| loader(name.as_ptr())) };
        match GlRenderer::new(Arc::new(context)) {
            Ok(renderer) => {
                editor.renderer = Some(renderer);
                std::ptr::null_mut()
            }
            Err(error) => into_c_string(error.to_string()),
        }
    })
}

/// Draws the current frame into the bound framebuffer. Returns null on success, or an error
/// message. With no project open it only clears the background.
///
/// # Safety
/// `handle` must be null or a live editor; the GL context must be current.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_editor_viewport_render(
    handle: *mut EditorHandle,
    width: u32,
    height: u32,
) -> *mut c_char {
    // SAFETY: forwarded contract.
    let Some(editor) = (unsafe { handle.as_mut() }) else {
        return into_c_string("o editor é nulo".to_string());
    };
    guarded(into_c_string, || {
        let Some(renderer) = editor.renderer.as_mut() else {
            return into_c_string("o viewport ainda não foi inicializado".to_string());
        };
        let frame = editor.session.frame().unwrap_or_else(empty_frame);
        match renderer.render(&frame, (width, height)) {
            Ok(()) => std::ptr::null_mut(),
            Err(error) => into_c_string(error.to_string()),
        }
    })
}

fn empty_frame() -> petunia_render::RenderFrame {
    petunia_render::RenderFrame {
        camera: petunia_render::Camera::default(),
        items: Vec::new(),
        sculpts: Vec::new(),
        overlay: Vec::new(),
        show_grid: false,
        grid_height: 0.0,
        clear_color: petunia_math::Color::rgb(0.16, 0.17, 0.20),
    }
}

/// Frees the renderer's GPU objects. Call with the GL context current (for example just before
/// the widget is destroyed).
///
/// # Safety
/// `handle` must be null or a live editor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_editor_viewport_release(handle: *mut EditorHandle) {
    // SAFETY: forwarded contract.
    if let Some(editor) = unsafe { handle.as_mut() } {
        editor.renderer = None;
    }
}

/// Releases a string returned by this library.
///
/// # Safety
/// `text` must be null or a pointer returned by this library, not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn petunia_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: the caller guarantees the pointer came from `CString::into_raw` here.
        drop(unsafe { CString::from_raw(text) });
    }
}
