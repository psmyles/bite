//! Reading a rendered image back to the CPU, which is what `--capture` writes a PNG from.
//!
//! sokol_gfx has no `read_pixels`: it is a drawing API, and getting pixels back is a
//! per-backend affair. So this reaches through to the native handle sokol_gfx keeps for the
//! image - `sg::d3d11_query_image_info` - copies the texture into a staging texture the CPU can
//! map, and reads the rows out of it, unpadding as it goes.
//!
//! Only what the capture needs: one RGBA8 2D image, one mip, one slice. The general version
//! (cube faces, mip levels, other formats) is 3d-review's `read_image_subresource`, which this is
//! reduced from.
//!
//! Nothing needs unbinding first: a sokol pass is closed by `sg::end_pass`, so by the time this
//! runs the image is no longer a bound render target.

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
    // SAFETY: sokol hands back borrowed COM pointers it owns for the life of the image and the
    // device; `from_raw_borrowed` takes no ownership, so nothing is released here. The image
    // outlives the call - the caller holds it until the PNG is written - and the capture is
    // single-threaded, so nothing else touches the immediate context.
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
    // SAFETY: a CPU-readable staging texture with no initial data; the out-param is populated on
    // success and checked below.
    unsafe { device.CreateTexture2D(&description, None, Some(&mut staging)) }
        .map_err(|error| format!("capture staging texture failed: {error}"))?;
    let staging = staging.ok_or("the capture staging texture was not created")?;

    // SAFETY: the source subresource matches the staging texture in format and size, and a null
    // box copies the whole of it.
    unsafe { context.CopySubresourceRegion(&staging, 0, 0, 0, 0, source, 0, None) };

    let row = width as usize * 4;
    let mut pixels = Vec::with_capacity(row * height as usize);
    // SAFETY: the staging texture is mappable; the mapped range is at least `RowPitch * height`
    // bytes, so every tight-row copy stays in bounds. `Unmap` is paired with `Map`.
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
