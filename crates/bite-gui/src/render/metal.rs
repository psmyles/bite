//! The Metal device and the `CAMetalLayer` sokol_gfx draws through on macOS.
//!
//! sokol_gfx does not own a window: it is handed a device at `sg_setup` (through
//! `sg_environment`) and a render target per frame (through `sg_swapchain`), and the shell owns
//! everything around them. This module is that glue - the twin of [`crate::render::d3d11`], same
//! shape, same contract - and the one place in the editor that names Metal or Core Animation.
//!
//! Three things differ from the D3D11 side, and each is load-bearing:
//!
//! * **The backbuffer is `BGRA8Unorm`, not `RGBA8Unorm`.** A `CAMetalLayer` accepts only a short
//!   list of formats and RGBA8 is not on it. That is a channel *order* difference in storage
//!   only: the shader still writes float4 RGBA and Metal swizzles on the way out, so the
//!   interface's colours are unchanged. It does reach one other place - [`readback`] reads the
//!   capture's pixels in this order and puts them back in RGBA order for the caller.
//!
//! * **sokol presents the drawable, not us.** `sg_end_pass` calls `presentDrawable:` on whatever
//!   `sg_swapchain` was pointed at and `sg_commit` commits the buffer, so [`Swapchain::present`]
//!   only releases our own hold on the drawable. Presenting again here would be a double present.
//!
//! * **The frame blocks at acquire, not at present.** `nextDrawable` waits until the display has
//!   taken an earlier frame; D3D11 waits inside `Present(1, 0)` instead. Either way the frame is
//!   paced by the display, which is what the editor's settle burst is counted against.
//!
//! The device is created on the bring-up thread (`platform::start_gpu`) and used from the main
//! thread after the join. The *layer* is not: Core Animation and `NSView` are main-thread-only,
//! so [`Swapchain::new`] must be called there, as its caller (`App::create`) is.
//!
//! Transplanted from fire's `render/metal.rs`, the same shell this editor's Windows half came
//! from.
//!
//! [`readback`]: crate::render::readback

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::NSView;
use objc2_foundation::CGSize;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use sokol::gfx as sg;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// What the backbuffer is created as, and what sokol_gfx is told to expect of a swapchain pass,
/// so the interface pipeline matches it. `BGRA8`, not the D3D11 side's `RGBA8`: see the module
/// comment.
pub const SWAPCHAIN_FORMAT: sg::PixelFormat = sg::PixelFormat::Bgra8;

/// How many drawables may be in flight. Two, to mirror the D3D11 side's two-buffer flip chain:
/// with three, `nextDrawable` only blocks once three frames are queued, which is a frame of
/// latency bought for smoothness an editor does not need.
const MAX_DRAWABLES: usize = 2;

/// The process's Metal device - what sokol_gfx runs on.
pub struct Device {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
}

// SAFETY: an `MTLDevice` is thread-safe by specification - Metal explicitly allows creating and
// using a device from any thread. The bring-up thread creates it and hands it to the main thread
// through a `JoinHandle`, whose join is the synchronization point.
unsafe impl Send for Device {}

impl Device {
    /// The system's default GPU.
    ///
    /// `force_warp` is the D3D11 side's software-rasterizer request (`BITE_GPU=warp`, for RDP)
    /// and is ignored here: Metal has no software device to fall back to, and every Mac that runs
    /// a supported macOS has a Metal-capable GPU. The parameter stays so the shell calls one
    /// signature on both platforms.
    ///
    /// Errors come back as strings for the caller to show. This runs at startup in a process that
    /// may have no console, where a panic is an invisible abort.
    pub fn create(_force_warp: bool) -> Result<Device, String> {
        // SAFETY: the standard Metal entry point, valid to call from any thread. It returns a
        // +1 reference (a `Create` rule function), so `Retained::from_raw` takes that ownership
        // rather than retaining again.
        let device = unsafe { Retained::from_raw(MTLCreateSystemDefaultDevice()) };
        device
            .map(|device| Device { device })
            .ok_or_else(|| "no Metal device: this GPU cannot run Bite".to_string())
    }

    /// Points `environment` at this device, so `sg_setup` runs on it.
    pub fn fill_environment(&self, environment: &mut sg::Environment) {
        environment.metal = sg::MetalEnvironment {
            // Not retained by the caller beyond the device's own lifetime: sokol_gfx retains
            // what it keeps.
            device: Retained::as_ptr(&self.device).cast::<c_void>(),
        };
    }
}

/// The window's `CAMetalLayer` and the drawable it is currently rendering into.
pub struct Swapchain {
    layer: Retained<CAMetalLayer>,
    /// This frame's drawable, held from [`Self::acquire`] to [`Self::present`]. sokol_gfx keeps
    /// only a borrowed pointer to it across the pass, so it must stay alive until the command
    /// buffer that presents it has been committed.
    drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    width: u32,
    height: u32,
}

impl Swapchain {
    /// Attaches a `width` x `height` Metal layer to `window`'s view: vsync-paced, two drawables
    /// in flight, `BGRA8Unorm` - the closest equivalent of the flip-model chain the D3D11 side
    /// makes.
    ///
    /// Must be called on the main thread: it touches `NSView` and Core Animation.
    pub fn new(device: &Device, window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let ns_view = match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::AppKit(handle)) => handle.ns_view,
            _ => return Err("the window has no AppKit handle".into()),
        };
        let (width, height) = (width.max(1), height.max(1));

        // SAFETY: `CAMetalLayer::new` is a plain `+[CAMetalLayer layer]`-style allocation, and
        // every setter below is a documented property on the layer it was just handed.
        let layer = unsafe {
            let layer = CAMetalLayer::new();
            layer.setDevice(Some(&device.device));
            layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            // The editor only ever renders into the drawable and never reads it back, which lets
            // Core Animation skip making it readable.
            layer.setFramebufferOnly(true);
            layer.setMaximumDrawableCount(MAX_DRAWABLES);
            // Vsync: what makes `nextDrawable` block, and so what paces the frame.
            layer.setDisplaySyncEnabled(true);
            layer.setContentsScale(window.scale_factor());
            layer.setDrawableSize(CGSize::new(f64::from(width), f64::from(height)));
            layer
        };

        // SAFETY: winit hands out a live `NSView` for the window it owns, and this runs on the
        // main thread (see the doc comment). Setting the layer *before* `setWantsLayer:` makes
        // the view layer-*hosting* - it draws only what we render - rather than layer-backed,
        // where AppKit would own and redraw the layer's contents itself.
        unsafe {
            let view: &NSView = ns_view.cast().as_ref();
            view.setLayer(Some(&layer));
            view.setWantsLayer(true);
        }

        Ok(Swapchain {
            layer,
            drawable: None,
            width,
            height,
        })
    }

    /// The backbuffer size, in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Resizes the layer's drawables.
    ///
    /// A zero dimension - a minimised window - is remembered but not applied, because Core
    /// Animation refuses it; the frame is skipped instead ([`Self::acquire`]).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        if width == 0 || height == 0 {
            return;
        }
        // SAFETY: a documented property on a live layer.
        unsafe {
            self.layer
                .setDrawableSize(CGSize::new(f64::from(width), f64::from(height)));
        }
    }

    /// Follows the window onto a display with a different backing scale.
    ///
    /// `contentsScale` is how Core Animation maps the layer's *point* bounds - which AppKit sets
    /// from the view - onto the drawable's pixels. [`Self::resize`] keeps the drawable itself
    /// right, but on its own that is not enough: left at the old display's scale, Core Animation
    /// would believe the layer needs twice (or half) the pixels the drawable actually has and
    /// rescale it to fit, so a window dragged from a Retina display to a 1x one would go soft
    /// rather than sharp. The two have to move together, and winit reports them as two events.
    ///
    /// The D3D11 twin has no counterpart - DXGI has no notion of a scale between the swapchain
    /// and the window - so its version of this does nothing.
    pub fn set_scale_factor(&mut self, scale: f64) {
        self.layer.setContentsScale(scale);
    }

    /// Acquires this frame's drawable and points `swapchain` at it, answering whether there is a
    /// frame to draw at all.
    ///
    /// `false` - a minimised or off-screen window, or a drawable that timed out - means the
    /// caller skips the frame rather than drawing into nothing.
    pub fn acquire(&mut self, swapchain: &mut sg::Swapchain) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        // SAFETY: a documented method on a live layer. `nextDrawable` returns nil rather than
        // blocking forever when the layer is off-screen or the wait times out.
        let drawable = unsafe { self.layer.nextDrawable() };
        let Some(drawable) = drawable else {
            self.drawable = None;
            return false;
        };
        swapchain.metal.current_drawable = Retained::as_ptr(&drawable).cast::<c_void>();
        // Held until `present`: sokol_gfx only borrows the pointer across the pass.
        self.drawable = Some(drawable);
        true
    }

    /// Finishes the frame.
    ///
    /// There is no present call here: sokol_gfx's `sg_end_pass` already scheduled
    /// `presentDrawable:` on the frame's command buffer and `sg_commit` committed it. All that is
    /// left is to release our own hold on the drawable, which the command buffer retains for as
    /// long as it needs.
    pub fn present(&mut self) {
        self.drawable = None;
    }
}
