//! The D3D11 device and the DXGI flip-model swapchain sokol_gfx draws through on Windows.
//!
//! sokol_gfx does not own a window: it is handed a device at `sg_setup` (through
//! `sg_environment`) and a render-target view per frame (through `sg_swapchain`), and the shell
//! owns everything around them. This module is that glue, and the one place in the editor that
//! names D3D11 or DXGI. Its macOS twin is [`crate::render::metal`] - a `CAMetalLayer` on the
//! winit window, handing sokol_gfx its `MTLDevice` and per-frame drawable - and the two keep the
//! same contract, which `render::mod` states.
//!
//! The backbuffer is plain `R8G8B8A8_UNORM`: the flip model disallows `*_SRGB` swapchain formats,
//! and the interface's colours are authored as sRGB already - which is also what the wgpu surface
//! this replaces deliberately asked for, so nothing about the editor's colour handling changes.
//!
//! The device is created on the bring-up thread and used from the main thread after the join:
//! D3D11 devices are free-threaded, and only the main thread ever touches the immediate context.
//!
//! Transplanted from fire's `render/d3d11.rs`, whose measurements are why this migration exists.

use std::ffi::c_void;

use crate::logging;
use sokol::gfx as sg;
use windows::core::Interface;
use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HWND};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// What the backbuffer is created as, and what sokol_gfx is told to expect of a swapchain pass,
/// so the interface pipeline matches it. Plain `RGBA8`, not sRGB: see the module comment.
pub const SWAPCHAIN_FORMAT: sg::PixelFormat = sg::PixelFormat::Rgba8;

/// The process's D3D11 device and its immediate context - what sokol_gfx runs on.
pub struct Device {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

// SAFETY: a D3D11 device is free-threaded by specification. The immediate context is not, and it
// is only ever used by the main thread - the bring-up thread creates both and hands them over
// through a `JoinHandle`, whose join is the synchronization point.
unsafe impl Send for Device {}

impl Device {
    /// Creates a hardware device, falling back to the WARP software rasterizer.
    ///
    /// The fallback is not theoretical: the editor is used over RDP, where there is no hardware
    /// device to be had. `force_warp` takes the same path deliberately, for `BITE_GPU=warp`.
    ///
    /// Errors come back as strings for the caller to show. This runs at startup in a process that
    /// may have no console, where a panic is an invisible abort.
    pub fn create(force_warp: bool) -> Result<Device, String> {
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let drivers: &[(_, bool)] = if force_warp {
            &[(D3D_DRIVER_TYPE_WARP, true)]
        } else {
            &[
                (D3D_DRIVER_TYPE_HARDWARE, false),
                (D3D_DRIVER_TYPE_WARP, true),
            ]
        };
        let mut last_error = String::from("no D3D11 driver");
        for (driver, is_warp) in drivers.iter().copied() {
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            // SAFETY: every out-pointer is a live local; the feature-level slice outlives the call.
            let created = unsafe {
                D3D11CreateDevice(
                    None,
                    driver,
                    Default::default(),
                    D3D11_CREATE_DEVICE_FLAG(0),
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )
            };
            match created {
                Ok(()) => {
                    if is_warp && !force_warp {
                        logging::warn(
                            "No hardware Direct3D 11 device; using the WARP software renderer",
                        );
                    }
                    // Success filled both out-params; the unwraps are unreachable by contract.
                    return Ok(Device {
                        device: device.unwrap(),
                        context: context.unwrap(),
                    });
                }
                Err(error) => last_error = format!("D3D11CreateDevice failed: {error}"),
            }
        }
        Err(last_error)
    }

    /// Points `environment` at this device and its immediate context, so `sg_setup` runs on them.
    pub fn fill_environment(&self, environment: &mut sg::Environment) {
        environment.d3d11 = sg::D3d11Environment {
            // Neither pointer is retained by the caller beyond the device's own lifetime:
            // sokol_gfx AddRefs what it keeps.
            device: self.device.as_raw(),
            device_context: self.context.as_raw(),
        };
    }
}

/// The window's flip-model swapchain and the render-target view of its current backbuffer.
pub struct Swapchain {
    device: ID3D11Device,
    swapchain: IDXGISwapChain1,
    /// The backbuffer's view, created on demand ([`Self::render_view`]) and dropped on resize.
    view: Option<ID3D11RenderTargetView>,
    width: u32,
    height: u32,
}

impl Swapchain {
    /// Creates a `width` x `height` swapchain on `window`'s client: vsync-paced, two buffers,
    /// `FLIP_DISCARD`.
    pub fn new(device: &Device, window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let handle = match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => HWND(handle.hwnd.get() as *mut c_void),
            _ => return Err("the window has no Win32 handle".into()),
        };
        let description = DXGI_SWAP_CHAIN_DESC1 {
            Width: width.max(1),
            Height: height.max(1),
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            ..Default::default()
        };
        // SAFETY: plain COM traversal device -> adapter -> factory on a live device; the
        // description is fully initialized and the handle is the caller's live window.
        let swapchain = unsafe {
            let dxgi: IDXGIDevice = device.device.cast().map_err(|e| e.to_string())?;
            let adapter: IDXGIAdapter = dxgi.GetAdapter().map_err(|e| e.to_string())?;
            let factory: IDXGIFactory2 = adapter.GetParent().map_err(|e| e.to_string())?;
            factory
                .CreateSwapChainForHwnd(&device.device, handle, &description, None, None)
                .map_err(|error| format!("CreateSwapChainForHwnd failed: {error}"))?
        };
        Ok(Swapchain {
            device: device.device.clone(),
            swapchain,
            view: None,
            width: width.max(1),
            height: height.max(1),
        })
    }

    /// The backbuffer size, in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Drops the view and resizes the backbuffers.
    ///
    /// A zero dimension - a minimised window - is remembered but not applied, because DXGI
    /// refuses it; the frame is skipped instead ([`Self::acquire`]).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.view = None;
        if width == 0 || height == 0 {
            return;
        }
        // SAFETY: the view - the only outstanding reference to a backbuffer - was dropped above.
        if let Err(error) = unsafe {
            self.swapchain.ResizeBuffers(
                0,
                width,
                height,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        } {
            // The backbuffers stay at their old size; the next frame draws into them anyway.
            logging::warn(format!("Swapchain resize failed: {error}"));
        }
    }

    /// Follows the window onto a display with a different backing scale, which on DXGI is
    /// nothing at all: a swapchain's buffers are sized in pixels and it has no notion of a scale
    /// between them and the window, so the resize the same event brings is the whole story. The
    /// Metal twin has real work to do here (`metal::Swapchain::set_scale_factor`), and the shell
    /// calls one method on both.
    pub fn set_scale_factor(&mut self, _scale: f64) {}

    /// Acquires this frame's render target and points `swapchain` at it, answering whether there
    /// is a frame to draw at all.
    ///
    /// `false` means either a minimised window or a device that refused, and the caller skips the
    /// frame rather than drawing into nothing.
    pub fn acquire(&mut self, swapchain: &mut sg::Swapchain) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        match self.render_view() {
            Some(view) => {
                swapchain.d3d11.render_view = view;
                true
            }
            None => false,
        }
    }

    /// The render-target view of the current backbuffer, as the raw pointer `sg_swapchain` takes,
    /// creating it if a resize dropped it. `None` - logged - if the device refuses.
    fn render_view(&mut self) -> Option<*const c_void> {
        if self.view.is_none() {
            // SAFETY: buffer 0 of a live flip-model swapchain; the out-pointer is a live local.
            self.view = unsafe {
                let back: ID3D11Texture2D = match self.swapchain.GetBuffer(0) {
                    Ok(buffer) => buffer,
                    Err(error) => {
                        logging::warn(format!("Swapchain buffer unavailable: {error}"));
                        return None;
                    }
                };
                let mut view: Option<ID3D11RenderTargetView> = None;
                match self
                    .device
                    .CreateRenderTargetView(&back, None, Some(&mut view))
                {
                    Ok(()) => view,
                    Err(error) => {
                        logging::warn(format!("Render target view failed: {error}"));
                        None
                    }
                }
            };
        }
        self.view
            .as_ref()
            .map(Interface::as_raw)
            .map(|pointer| pointer as *const c_void)
    }

    /// Presents the completed frame, vsync-paced.
    ///
    /// `DXGI_STATUS_OCCLUDED` - the window hidden or fully covered - is a success that returns
    /// immediately rather than waiting for the display, which is worth not treating as an error.
    pub fn present(&mut self) {
        // SAFETY: a live swapchain; sync interval 1 with no flags is always valid.
        let result = unsafe { self.swapchain.Present(1, DXGI_PRESENT(0)) };
        if result.is_err() && result != DXGI_STATUS_OCCLUDED {
            logging::warn(format!("Present failed: {result:?}"));
        }
    }
}
