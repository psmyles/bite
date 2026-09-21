//! Reading a rendered image back to the CPU, which is what `--capture` writes a PNG from.
//!
//! sokol_gfx has no `read_pixels`: it is a drawing API, and getting pixels back is a per-backend
//! affair. So each arm below reaches through to the native handle sokol_gfx keeps for the image
//! and copies it into something the CPU can read - a staging texture on D3D11, a shared buffer on
//! Metal - then hands the caller tightly packed **RGBA8**, whatever order the backend stored it
//! in.
//!
//! That last part is the one thing the two arms do not share. The capture target is created with
//! [`SWAPCHAIN_FORMAT`], because sokol_imgui's pipeline is built once with that format and a pass
//! whose attachment disagrees fails validation - and on macOS that format is `BGRA8`, since a
//! `CAMetalLayer` will not take `RGBA8` (see [`crate::render::metal`]). So the Metal arm swaps the
//! red and blue bytes on the way out and the goldens stay one set of RGBA images on both
//! platforms.
//!
//! Only what the capture needs: one 2D image, one mip, one slice.
//!
//! Nothing needs unbinding first: a sokol pass is closed by `sg::end_pass`, so by the time this
//! runs the image is no longer a bound render target.
//!
//! [`SWAPCHAIN_FORMAT`]: crate::render::SWAPCHAIN_FORMAT

#[cfg(windows)]
pub use d3d11::rgba8;
#[cfg(target_os = "macos")]
pub use metal::rgba8;

/// Copies `image` back through a CPU-readable staging texture, row by row.
#[cfg(windows)]
mod d3d11 {
    use sokol::gfx as sg;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::core::Interface;

    /// Copies `image` back to the CPU as tightly packed RGBA8, row by row.
    pub fn rgba8(image: sg::Image, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let info = sg::d3d11_query_image_info(image);
        if info.tex2d.is_null() {
            return Err("the capture target is not a Direct3D 11 2D texture".into());
        }
        // sokol's native-handle getters are `*const` and `from_raw_borrowed` wants `&*mut`; the
        // constness carries no meaning across the C ABI here.
        let texture = info.tex2d.cast_mut();
        let device = sg::d3d11_device().cast_mut();
        let context = sg::d3d11_device_context().cast_mut();
        // SAFETY: sokol hands back borrowed COM pointers it owns for the life of the image and
        // the device; `from_raw_borrowed` takes no ownership, so nothing is released here. The
        // image outlives the call - the caller holds it until the PNG is written - and the
        // capture is single-threaded, so nothing else touches the immediate context.
        let (source, device, context) = unsafe {
            (
                ID3D11Texture2D::from_raw_borrowed(&texture),
                ID3D11Device::from_raw_borrowed(&device),
                ID3D11DeviceContext::from_raw_borrowed(&context),
            )
        };
        let (Some(source), Some(device), Some(context)) = (source, device, context) else {
            return Err("no Direct3D 11 texture, device or context to read back from".into());
        };

        let description = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging = None;
        // SAFETY: a CPU-readable staging texture with no initial data; the out-param is
        // populated on success and checked below.
        unsafe { device.CreateTexture2D(&description, None, Some(&mut staging)) }
            .map_err(|error| format!("capture staging texture failed: {error}"))?;
        let staging = staging.ok_or("the capture staging texture was not created")?;

        // SAFETY: the source subresource matches the staging texture in format and size, and a
        // null box copies the whole of it.
        unsafe { context.CopySubresourceRegion(&staging, 0, 0, 0, 0, source, 0, None) };

        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        // SAFETY: the staging texture is mappable; the mapped range is at least
        // `RowPitch * height` bytes, so every tight-row copy stays in bounds. `Unmap` is paired
        // with `Map`.
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|error| format!("capture readback map failed: {error}"))?;
            let base = mapped.pData as *const u8;
            // The GPU's rows are padded to a hardware alignment; the caller wants them tight.
            let pitch = mapped.RowPitch as usize;
            for index in 0..height as usize {
                pixels.extend_from_slice(std::slice::from_raw_parts(base.add(index * pitch), row));
            }
            context.Unmap(&staging, 0);
        }
        Ok(pixels)
    }
}

/// Copies `image` back through a blit into a shared buffer.
#[cfg(target_os = "macos")]
mod metal {
    use objc2::runtime::ProtocolObject;
    use objc2_metal::{
        MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
        MTLDevice, MTLOrigin, MTLResourceOptions, MTLSize, MTLTexture,
    };
    use sokol::gfx as sg;

    /// Copies `image` back to the CPU as tightly packed RGBA8.
    ///
    /// A blit, not a `getBytes`: sokol creates every colour-attachment image with
    /// `MTLResourceStorageModePrivate` (`sokol_gfx.h`, `_sg_mtl_init_texdesc_common`), so the
    /// texture's contents are not addressable by the CPU at all. A blit encoder is the supported
    /// way across that line, into a `StorageModeShared` buffer that both sides can see.
    pub fn rgba8(image: sg::Image, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let info = sg::mtl_query_image_info(image);
        let slot = info.active_slot.clamp(0, info.tex.len() as i32 - 1) as usize;
        let texture = info.tex[slot];
        if texture.is_null() {
            return Err("the capture target is not a Metal texture".into());
        }
        let device = sg::mtl_device();
        let queue = sg::mtl_command_queue();
        if device.is_null() || queue.is_null() {
            return Err("no Metal device or command queue to read back through".into());
        }
        // SAFETY: sokol owns these three objects for the life of the image and the device, and
        // hands back borrowed pointers to them. Casting a pointer to a `&ProtocolObject` borrows
        // without retaining, which is what is wanted: nothing is released here. The image
        // outlives the call - the caller holds it until the PNG is written.
        let (texture, device, queue) = unsafe {
            (
                &*texture.cast::<ProtocolObject<dyn MTLTexture>>(),
                &*device.cast::<ProtocolObject<dyn MTLDevice>>(),
                &*queue.cast::<ProtocolObject<dyn MTLCommandQueue>>(),
            )
        };

        let row = width as usize * 4;
        let bytes = row * height as usize;
        // Shared storage is what makes `contents()` readable from this side with no explicit
        // synchronize.
        let buffer = device
            .newBufferWithLength_options(bytes, MTLResourceOptions::MTLResourceStorageModeShared)
            .ok_or("the capture readback buffer could not be allocated")?;

        let commands = queue
            .commandBuffer()
            .ok_or("the capture command buffer could not be created")?;
        let blit = commands
            .blitCommandEncoder()
            .ok_or("the capture blit encoder could not be created")?;
        // SAFETY: slice 0 / level 0 of a 2D texture the caller says is `width` x `height`, into a
        // buffer allocated at exactly the tight size this pitch describes, so the write stays in
        // bounds at both ends.
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                texture,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize {
                    width: width as usize,
                    height: height as usize,
                    depth: 1,
                },
                &buffer,
                0,
                row,
                bytes,
            );
        }
        blit.endEncoding();
        commands.commit();
        // The capture is the only thing this process is doing, and the PNG cannot be written
        // before the copy lands, so blocking here is the whole point rather than a cost.
        // SAFETY: a committed command buffer; waiting on it is always valid.
        unsafe { commands.waitUntilCompleted() };

        // SAFETY: the buffer is shared storage of exactly `bytes` bytes and the blit above has
        // completed, so the whole range is initialized and readable.
        let mut pixels =
            unsafe { std::slice::from_raw_parts(buffer.contents().as_ptr().cast::<u8>(), bytes) }
                .to_vec();
        // The target is `SWAPCHAIN_FORMAT`, which on macOS is BGRA8 - see the module comment.
        // Every caller wants RGBA, so red and blue swap here rather than in each of them.
        if super::super::SWAPCHAIN_FORMAT == sg::PixelFormat::Bgra8 {
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Ok(pixels)
    }
}
