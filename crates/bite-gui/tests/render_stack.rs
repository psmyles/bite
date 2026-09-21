//! The graphics stack, brought up and read back without a window.
//!
//! Everything under `render/` is glue to things that only report failure at runtime: a device
//! that refuses, a `sg_environment` filled in wrongly, a `simgui_desc_t` whose fields do not line
//! up with the header's, a staging texture read at the wrong pitch. None of that is a compile
//! error, and all of it would otherwise first show up tangled together in a window that draws
//! nothing.
//!
//! So this walks the whole path once - device, sokol_gfx on it, sokol_imgui on that, an offscreen
//! pass cleared to a known colour, and the pixels read back - and checks the colour that comes
//! out. It is one test, not several, because `sg::setup` is process-wide and a test binary runs
//! its tests in one process.
//!
//! On a machine with no Direct3D 11 at all the device creation falls back to WARP, which is the
//! same fallback the editor ships for RDP; on macOS every supported Mac has a Metal device. So
//! there is no configuration on either platform in which this is expected to be skipped.
//!
//! It runs on both because the backends differ in exactly the way this checks. The macOS
//! swapchain format is `BGRA8` where Windows is `RGBA8`, and `readback` puts the channels back
//! in RGBA order for the caller - so the colour assertions below are what catches a swizzle that
//! was missed or applied twice, which is why [`CLEAR`] and [`RECT`] have no two channels alike.

#![cfg(any(windows, target_os = "macos"))]

use bite_gui::render::{Device, SWAPCHAIN_FORMAT, imgui, readback};
use sokol::gfx as sg;

/// The clear colour, chosen so every channel is distinct and none is 0 or 255 - a readback that
/// lost a channel, swapped two, or read the wrong rows would still pass against grey.
const CLEAR: [f32; 4] = [0.25, 0.5, 0.75, 1.0];
/// The colour of the rectangle the interface draws over the top-left quadrant, distinct from
/// [`CLEAR`] in every channel and fully opaque, so "the rectangle landed" and "the clear showed
/// through" can never be confused.
const RECT: [f32; 4] = [1.0, 0.0, 0.5, 1.0];
const SIZE: u32 = 64;
const HALF: u32 = SIZE / 2;

#[test]
fn the_graphics_stack_comes_up_and_reads_back_what_it_drew() {
    let device = Device::create(false).expect("a Direct3D 11 device, hardware or WARP");

    let mut description = sg::Desc::new();
    description.environment.defaults = sg::EnvironmentDefaults {
        color_format: SWAPCHAIN_FORMAT,
        depth_format: sg::PixelFormat::None,
        sample_count: 1,
    };
    device.fill_environment(&mut description.environment);
    description.logger = sg::Logger {
        func: Some(sokol::log::slog_func),
        user_data: std::ptr::null_mut(),
    };
    sg::setup(&description);
    assert!(sg::isvalid(), "sokol_gfx could not be set up on the device");

    // SAFETY: sokol_gfx is up and no ImGui frame is open. This is the call whose `simgui_desc_t`
    // mirror would be read as garbage if the struct had drifted from the header.
    unsafe { imgui::setup_once(SWAPCHAIN_FORMAT) };

    let mut image = sg::ImageDesc::new();
    image._type = sg::ImageType::Dim2;
    image.usage.color_attachment = true;
    image.width = SIZE as i32;
    image.height = SIZE as i32;
    image.num_mipmaps = 1;
    image.pixel_format = SWAPCHAIN_FORMAT;
    image.label = c"stack test target".as_ptr();
    let image = sg::make_image(&image);
    assert_eq!(sg::query_image_state(image), sg::ResourceState::Valid);

    let mut view = sg::ViewDesc::new();
    view.color_attachment.image = image;
    view.label = c"stack test target".as_ptr();
    let view = sg::make_view(&view);
    assert_eq!(sg::query_view_state(view), sg::ResourceState::Valid);

    // The interface, drawn by sokol_imgui into the same pass. This is the piece with the most
    // ways to be silently wrong - the frame handed over without `igRender` running twice, the
    // glyph and geometry uploads, the pipeline built from embedded bytecode - and the check is
    // that a rectangle of a known colour lands where it was asked for.
    let mut context = bite_imgui::Context::new(1.0).expect("an ImGui context");

    let mut pass = sg::Pass::new();
    pass.attachments.colors[0] = view;
    pass.action.colors[0] = sg::ColorAttachmentAction {
        load_action: sg::LoadAction::Clear,
        store_action: sg::StoreAction::Store,
        clear_value: sg::Color {
            r: CLEAR[0],
            g: CLEAR[1],
            b: CLEAR[2],
            a: CLEAR[3],
        },
    };
    let mut frame = context.frame(SIZE as f32, SIZE as f32, 1.0, 1.0 / 60.0);
    frame.ui().background_draw_list().rect(
        [0.0, 0.0],
        [HALF as f32, HALF as f32],
        bite_imgui::Color(RECT),
        0.0,
        bite_imgui::Rounding::None,
    );
    frame.submit();

    sg::begin_pass(&pass);
    // SAFETY: a frame is open on the current context and a pass is open, which is the contract.
    unsafe { imgui::render() };
    sg::end_pass();
    sg::commit();

    let pixels = readback::rgba8(image, SIZE, SIZE).expect("the target reads back");
    assert_eq!(
        pixels.len(),
        (SIZE * SIZE * 4) as usize,
        "the readback must be tightly packed, with the row padding removed"
    );

    let at = |x: u32, y: u32| {
        let index = (y * SIZE + x) as usize * 4;
        pixels[index..index + 4].to_vec()
    };
    let close = |got: &[u8], want: &[u8], what: &str| {
        for (channel, (got, want)) in got.iter().zip(want).enumerate() {
            assert!(
                got.abs_diff(*want) <= 1,
                "{what}: channel {channel} read {got}, wanted {want} (all: {got:?})"
            );
        }
    };

    // The three corners the rectangle does not cover are the clear colour, straight through. The
    // swapchain format is UNORM, so a float clear of 0.25 lands on 64 with no sRGB curve between:
    // one count of rounding either way is the encoder's, anything more is a colour space nobody
    // asked for.
    let expected: Vec<u8> = CLEAR.iter().map(|c| (c * 255.0).round() as u8).collect();
    close(&at(SIZE - 1, 0), &expected, "top-right");
    close(&at(0, SIZE - 1), &expected, "bottom-left");
    close(&at(SIZE - 1, SIZE - 1), &expected, "bottom-right");

    // The quadrant the interface drew is the rectangle's colour: sokol_imgui built its pipeline,
    // uploaded the geometry and drew it into the pass, and the rows came back the right way up.
    let rectangle: Vec<u8> = RECT.iter().map(|c| (c * 255.0).round() as u8).collect();
    close(&at(1, 1), &rectangle, "top-left inside the rectangle");
    close(&at(HALF - 2, HALF - 2), &rectangle, "inside, near its corner");

    // Note what is *not* called here: `simgui_shutdown`. It ends by destroying the *current*
    // ImGui context - bite's - which `Context`'s own `Drop` would then free a second time. See
    // the note in `render::imgui`; this test is what found it.
    sg::destroy_view(view);
    sg::destroy_image(image);
    sg::shutdown();
    drop(context);
}
