//! sokol_imgui: the renderer Dear ImGui draws through.
//!
//! `simgui/simgui.c` compiles `sokol_imgui.h` in its `SOKOL_IMGUI_NO_SOKOL_APP` mode - a
//! renderer only, since the window, the event loop and input are winit's. It uploads ImGui's
//! textures (fonts included, through the 1.92 texture protocol) as sokol_gfx images and draws the
//! draw lists into whatever pass is currently open. This module is the Rust side of that: the
//! declarations, the once-per-process setup, and the identifier an `sg::View` is named by.
//!
//! Three things here are load-bearing:
//!
//! * **The struct below must mirror `simgui_desc_t` exactly** - field order and types. Nothing
//!   checks it; a mismatch is read as garbage.
//! * **`simgui_setup` creates and destroys a context of its own.** Every later `simgui_*` call
//!   works on whichever context is current (`igGetIO()`), so bite's one context has to be made
//!   current again afterwards, and the backend flags have to be set on *it* rather than assumed
//!   from setup. `bite_imgui::Context` sets them itself, which is why setup can run on the
//!   bring-up thread before that context exists.
//! * **No shader is compiled at launch.** sokol_imgui carries its shaders as embedded bytecode
//!   for every backend, which is one of the two things this migration is for.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use sokol::gfx as sg;

#[repr(C)]
struct SimguiAllocator {
    alloc_fn: Option<unsafe extern "C" fn(usize, *mut c_void) -> *mut c_void>,
    free_fn: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    user_data: *mut c_void,
}

#[repr(C)]
struct SimguiLogger {
    func: Option<
        extern "C" fn(
            *const std::ffi::c_char,
            u32,
            u32,
            *const std::ffi::c_char,
            u32,
            *const std::ffi::c_char,
            *mut c_void,
        ),
    >,
    user_data: *mut c_void,
}

/// `simgui_desc_t`. Field order and types mirror `sokol_imgui.h` exactly.
#[repr(C)]
struct SimguiDesc {
    max_vertices: i32,
    color_format: sg::PixelFormat,
    depth_format: sg::PixelFormat,
    sample_count: i32,
    ini_filename: *const std::ffi::c_char,
    no_default_font: bool,
    disable_paste_override: bool,
    disable_set_mouse_cursor: bool,
    disable_windows_resize_from_edges: bool,
    write_alpha_channel: bool,
    allocator: SimguiAllocator,
    logger: SimguiLogger,
}

unsafe extern "C" {
    fn simgui_setup(desc: *const SimguiDesc);
    fn simgui_render();
    fn simgui_imtextureid(view: sg::View) -> u64;
}

/// How many vertices one frame may draw.
///
/// sokol_imgui sizes its vertex and index buffers once, from this. The default is 65536, which is
/// comfortable for the editor's chrome but not obviously enough for a filmstrip of hundreds of
/// thumbnails with rounded corners - and the failure mode is a frame that silently stops drawing
/// past a point rather than an error. Four times the default costs 20 bytes a vertex, so a little
/// over 5 MB, which is cheap against the working-set budget and against having to diagnose it.
const MAX_VERTICES: i32 = 65536 * 4;

/// Whether [`setup_once`] has run. Its sokol_gfx resources are process-wide, so a second call
/// would leak the first set and reset the vertex buffers mid-flight.
static UP: AtomicBool = AtomicBool::new(false);

/// Sets sokol_imgui up on the current sokol_gfx, once per process.
///
/// Safe to call before bite's ImGui context exists - it needs sokol_gfx, not a context, which is
/// what lets it run on the bring-up thread. It does create a context of its own, which is
/// destroyed here and leaves *none* current: between this call and the first
/// `bite_imgui::Context::new`, any ImGui call would read through a null global. Nothing runs in
/// that window - this is the last thing the bring-up thread does, and the context is made on the
/// main thread after the join.
///
/// # Safety
///
/// sokol_gfx must be set up, and no ImGui frame may be open.
pub unsafe fn setup_once(color_format: sg::PixelFormat) {
    if UP.swap(true, Ordering::SeqCst) {
        return;
    }
    let desc = SimguiDesc {
        max_vertices: MAX_VERTICES,
        color_format,
        depth_format: sg::PixelFormat::None,
        sample_count: 1,
        // The editor keeps no imgui.ini: its layout is its own, in `persist::Session`.
        ini_filename: std::ptr::null(),
        // The faces are registered by `bite_imgui::Fonts`; the built-in ProggyClean would only
        // cost an atlas nothing draws from.
        no_default_font: true,
        disable_paste_override: false,
        // The cursor shape is set from `Context::mouse_cursor` on the winit window.
        disable_set_mouse_cursor: true,
        disable_windows_resize_from_edges: false,
        write_alpha_channel: false,
        allocator: SimguiAllocator {
            alloc_fn: None,
            free_fn: None,
            user_data: std::ptr::null_mut(),
        },
        logger: SimguiLogger {
            func: Some(sokol::log::slog_func),
            user_data: std::ptr::null_mut(),
        },
    };
    // SAFETY: the description is fully initialized, and sokol_gfx is set up by the caller's
    // contract. The context sokol_imgui creates is destroyed here, before anything else runs;
    // sokol_imgui holds no reference to it.
    unsafe {
        simgui_setup(&desc);
        bite_imgui::discard_current_context();
    }
}

/// Ends the frame and draws it into the pass that is open.
///
/// This is `igRender` plus the texture uploads plus the draw calls - everything the owned wgpu
/// renderer used to do, and the reason there is no `shader.wgsl` any more.
///
/// # Safety
///
/// An ImGui frame must be open on the current context, and a sokol_gfx pass must be open.
pub unsafe fn render() {
    unsafe { simgui_render() }
}

// There is deliberately no `shutdown` here, although `sokol_imgui.h` exports one.
//
// `simgui_shutdown` ends by calling `igDestroyContext(NULL)`, which destroys whatever context is
// *current* - and at exit that is bite's, owned by a `bite_imgui::Context` whose own `Drop` then
// frees it a second time. The crash that follows is a heap corruption inside `igDestroyContext`,
// several frames from anything that looks like the cause; `tests/render_stack.rs` is what found
// it. fire reaches the same conclusion in its own teardown comment.
//
// What is given up by not calling it: sokol_imgui's pipelines, shaders, sampler, two buffers and
// its ~7 MB of vertex staging. All of it is process-wide, created once, and released by the
// process exiting - which is the only time this would have been called. `sg::shutdown` drops the
// sokol_gfx resources regardless.

/// The `ImTextureID` a view is drawn through.
///
/// This is how an image the editor uploaded gets into a draw list: the identifier the interface
/// stores is what sokol_imgui turns back into the view when it meets the draw command.
pub fn texture_id(view: sg::View) -> u64 {
    // SAFETY: a plain id computation over a handle; valid or not, it only reads the handle.
    unsafe { simgui_imtextureid(view) }
}
