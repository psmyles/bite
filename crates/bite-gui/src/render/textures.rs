//! The interface's image store: every texture the editor uploaded, by the key it knows it by.
//!
//! Two kinds of image reach the screen and only one of them is here. Dear ImGui's own textures -
//! the font atlas - are sokol_imgui's, created and freed inside `simgui_render` through the 1.92
//! texture protocol, and nothing in the editor ever names them. These are the other kind: the
//! thumbnails and previews the editor decodes, which it uploads itself and refers to by an
//! identifier it computes from a string (`app::texture_id`).
//!
//! That identifier is *not* what a draw list carries. An `sg::View` names a texture to
//! sokol_imgui, and the number that survives into an `ImDrawCmd` is
//! [`crate::render::imgui::texture_id`] of that view. So an entry holds both: the key the editor
//! frees by, and the id the interface draws by.

use std::collections::BTreeMap;

use sokol::gfx as sg;

use crate::logging;
use crate::render::imgui;

/// One uploaded image: the sokol resources and the identifier the interface draws it by.
///
/// The image and its view must outlive every frame that refers to the id, so they are only ever
/// replaced wholesale - an upload to a key that already exists destroys the old pair first.
struct Texture {
    image: sg::Image,
    view: sg::View,
    id: u64,
}

impl Texture {
    fn destroy(&self) {
        // Only while sokol_gfx is still up; the shell shuts it down after the store is gone.
        if sg::isvalid() {
            sg::destroy_view(self.view);
            sg::destroy_image(self.image);
        }
    }
}

/// Every image the editor has uploaded, by its key.
#[derive(Default)]
pub struct Textures {
    map: BTreeMap<u64, Texture>,
}

impl Textures {
    /// Uploads `pixels` as an RGBA8 image under `key`, answering the identifier the interface
    /// draws it by - the one that goes into a thumbnail or a preview.
    ///
    /// Zero on failure, which the interface draws as no image at all. A texture that could not be
    /// created is not a reason to take the editor down.
    pub fn upload(&mut self, key: u64, width: u32, height: u32, pixels: &[u8]) -> u64 {
        self.free(key);
        if width == 0 || height == 0 {
            return 0;
        }

        let mut description = sg::ImageDesc::new();
        description._type = sg::ImageType::Dim2;
        description.width = width as i32;
        description.height = height as i32;
        description.num_mipmaps = 1;
        description.pixel_format = sg::PixelFormat::Rgba8;
        description.data.mip_levels[0] = sg::slice_as_range(pixels);
        description.label = c"bite ui image".as_ptr();
        let image = sg::make_image(&description);

        let mut view = sg::ViewDesc::new();
        view.texture.image = image;
        view.label = c"bite ui image".as_ptr();
        let view = sg::make_view(&view);

        if sg::query_image_state(image) != sg::ResourceState::Valid
            || sg::query_view_state(view) != sg::ResourceState::Valid
        {
            logging::warn(format!("A {width}x{height} image could not be uploaded"));
            sg::destroy_view(view);
            sg::destroy_image(image);
            return 0;
        }

        let id = imgui::texture_id(view);
        self.map.insert(key, Texture { image, view, id });
        id
    }

    /// The identifier the interface draws `key` by, if it is uploaded.
    pub fn id(&self, key: u64) -> Option<u64> {
        self.map.get(&key).map(|texture| texture.id)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.map.contains_key(&key)
    }

    /// Releases one image, which the filmstrip does when a branch's thumbnails are replaced.
    pub fn free(&mut self, key: u64) {
        if let Some(texture) = self.map.remove(&key) {
            texture.destroy();
        }
    }

    /// Releases every image whose key the predicate rejects.
    pub fn free_where(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.map.retain(|key, texture| {
            let keeping = keep(*key);
            if !keeping {
                texture.destroy();
            }
            keeping
        });
    }
}

impl Drop for Textures {
    fn drop(&mut self) {
        for texture in self.map.values() {
            texture.destroy();
        }
    }
}
