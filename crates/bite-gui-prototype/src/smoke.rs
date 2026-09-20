//! Offscreen rendering, used to capture the interface for side-by-side comparison.
use crate::{app, renderer::Renderer, theme};
use bite_imgui::Context;
use std::path::{Path, PathBuf};

/// A scenario to capture, so that each comparison shot is reproducible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    /// The seed workflow, exactly as a new document opens.
    Seed,
    /// A workflow with a node selected, so the inspector is populated.
    Selected,
    /// The node creation menu open on the canvas.
    CreateMenu,
    /// The unsaved-changes prompt.
    ConfirmPrompt,
    /// The batch summary dialog with a representative result.
    BatchSummary,
    /// The run dialog with one ready and one blocked output.
    RunDialog,
    /// The about dialog.
    About,
    /// The credits dialog.
    Credits,
    /// The update dialog reporting a newer release.
    Update,
    /// A comment card, whose heading and body are drawn on the canvas.
    Comment,
    /// A node with a slider parameter selected, so the inspector shows the slider row.
    Slider,
    /// A node with a colour parameter selected, so the inspector shows the swatch row.
    Color,
    /// A column of process cards, for checking card widths, rows and ports.
    Cards,
    /// A menu bar dropdown held open, for checking its row spacing.
    Menu,
    /// An inspector dropdown held open, for checking its row spacing.
    Dropdown,
}

impl Scene {
    /// The scene a command line name selects.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "seed" => Self::Seed,
            "selected" => Self::Selected,
            "create-menu" => Self::CreateMenu,
            "confirm" => Self::ConfirmPrompt,
            "summary" => Self::BatchSummary,
            "run-dialog" => Self::RunDialog,
            "about" => Self::About,
            "credits" => Self::Credits,
            "update" => Self::Update,
            "comment" => Self::Comment,
            "slider" => Self::Slider,
            "color" => Self::Color,
            "cards" => Self::Cards,
            "menu" => Self::Menu,
            "dropdown" => Self::Dropdown,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Selected => "selected",
            Self::CreateMenu => "create-menu",
            Self::ConfirmPrompt => "confirm",
            Self::BatchSummary => "summary",
            Self::RunDialog => "run-dialog",
            Self::About => "about",
            Self::Credits => "credits",
            Self::Update => "update",
            Self::Comment => "comment",
            Self::Slider => "slider",
            Self::Color => "color",
            Self::Cards => "cards",
            Self::Menu => "menu",
            Self::Dropdown => "dropdown",
        }
    }

    /// Every scene, for a full capture run.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Seed,
            Self::Selected,
            Self::CreateMenu,
            Self::ConfirmPrompt,
            Self::BatchSummary,
            Self::RunDialog,
            Self::About,
            Self::Credits,
            Self::Update,
            Self::Comment,
            Self::Slider,
            Self::Color,
            Self::Cards,
            Self::Menu,
            Self::Dropdown,
        ]
    }
}

/// Where the pointer rests to hold the File menu open for its capture.
const MENU_POINTER: [f32; 2] = [96.0, 15.0];

/// Where the pointer rests to hold the inspector's list open for its capture.
const DROPDOWN_POINTER: [f32; 2] = [1450.0, 180.0];

/// Arranges the editor for a scene.
fn stage(editor: &mut app::Editor, scene: Scene, workflow: Option<&Path>) {
    if let Some(path) = workflow {
        crate::commands::open_path(editor, path);
    }
    editor.sync_active_input();
    match scene {
        Scene::Seed => {}
        Scene::Selected => {
            // The first processing node, or failing that the first node at all.
            let target = editor
                .studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| {
                    matches!(
                        node.kind,
                        bite_schema::NodeKind::Processing(
                            bite_schema::ProcessingNodeKind::Process
                        )
                    )
                })
                .or_else(|| editor.studio.workflow.graph.nodes.first())
                .map(|node| node.id.clone());
            if let Some(target) = target {
                editor.canvas.state.select_only([target.clone()]);
                editor.selected_node = Some(target);
                editor.sync_active_input();
            }
        }
        Scene::CreateMenu => {
            editor.create_menu.open_at([120.0, 120.0], None);
            editor.create_menu.can_group = true;
        }
        Scene::ConfirmPrompt => {
            editor.modal = crate::modals::Modal::Confirm {
                message: crate::modals::confirm_message(crate::modals::PendingAction::New),
                pending: crate::modals::PendingAction::New,
            };
        }
        Scene::BatchSummary => {
            editor.modal = crate::modals::Modal::BatchSummary(crate::modals::BatchSummary {
                processed: 24,
                skipped: 2,
                failed: 1,
                elapsed_ms: Some(4230),
                errors: vec![
                    "corrupted_scan.jpg: decode error - unsupported colour space".into(),
                ],
                output_dir: Some("out".into()),
            });
        }
        Scene::RunDialog => {
            editor.modal = crate::modals::Modal::RunWorkflow {
                nodes: vec![
                    crate::modals::RunCandidate {
                        id: "a".into(),
                        label: "Image Output".into(),
                        reasons: Vec::new(),
                    },
                    crate::modals::RunCandidate {
                        id: "b".into(),
                        label: "Text Output".into(),
                        reasons: vec!["Output file path is empty".into()],
                    },
                ],
            };
        }
        Scene::About => {
            editor.modal = crate::modals::Modal::About {
                versions: vec![
                    ("ImageMagick".into(), "7.1.1".into()),
                    ("Dear ImGui".into(), "1.90.9".into()),
                    ("wgpu".into(), "30.0".into()),
                    ("winit".into(), "0.30".into()),
                ],
            };
        }
        Scene::Comment => {
            let id = editor.studio.add_comment(bite_schema::Position {
                x: 120.0,
                y: 360.0,
            });
            editor.studio.set_param(
                &id,
                "heading".into(),
                bite_schema::ParamValue::String("Release checklist".into()),
            );
            editor.studio.set_param(
                &id,
                "body".into(),
                bite_schema::ParamValue::String(
                    "Resize to 2048, strip metadata, then convert to WEBP before the                      flipbook is built."
                        .into(),
                ),
            );
        }
        Scene::Slider | Scene::Color | Scene::Dropdown => {
            // Sharpen carries two sliders, Tint a colour and a slider, Compare a list.
            let definition = match scene {
                Scene::Slider => "sharpen",
                Scene::Color => "tint",
                _ => "logic_comparison",
            };
            let position = bite_schema::Position { x: 320.0, y: 360.0 };
            if let Ok(id) = editor.studio.add_processing(definition, position) {
                editor.canvas.state.select_only([id.clone()]);
                editor.selected_node = Some(id);
            }
        }
        Scene::Cards => {
            // Cards whose labels, rows and ports each exercise a different layout rule:
            // a long header, an enum-only definition, a channel count and a computed row.
            let definitions = [
                "premultiply-alpha",
                "channel_split",
                "brightness_contrast",
                "channel_merge",
                "outline",
                "format_convert",
            ];
            for (index, definition) in definitions.iter().enumerate() {
                let position = bite_schema::Position {
                    x: 120.0,
                    y: 40.0 + index as f64 * 130.0,
                };
                let _ = editor.studio.add_processing(definition, position);
            }
            editor.canvas.state.viewport.y = -20.0;
        }
        Scene::Menu => {}
        Scene::Credits => editor.modal = crate::modals::Modal::Credits,
        Scene::Update => {
            editor.modal = crate::modals::Modal::Update(crate::modals::UpdateState::Available {
                version: "9.9.9".into(),
                body: "Faster imports and a reworked inspector.".into(),
                url: "https://example.invalid/release".into(),
            });
        }
    }
}

/// Renders one scene to a portable network graphic.
pub fn capture(
    scene: Scene,
    output: &Path,
    workflow: Option<&Path>,
    size: (u32, u32),
    scale: f32,
) -> Result<(), String> {
    let mut editor = app::Editor::new()?;
    stage(&mut editor, scene, workflow);

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|error| error.to_string())?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = pollster::block_on(Renderer::new(&adapter, format))?;

    let mut context = Context::new(scale)?;
    theme::apply_base_style();
    let (width, height, pixels) = context.fonts().texture();
    renderer.texture(1, width, height, &pixels);
    context.fonts().set_texture_id(1);

    let physical = (
        (size.0 as f32 * scale) as u32,
        (size.1 as f32 * scale) as u32,
    );
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen capture"),
        size: wgpu::Extent3d {
            width: physical.0,
            height: physical.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());

    // A few frames let hover states, layout and the settle burst reach a steady state.
    let logical = [size.0 as f32, size.1 as f32];
    let mut data = Vec::new();
    for frame in 0..6 {
        if scene == Scene::Dropdown {
            context.mouse_position(DROPDOWN_POINTER[0], DROPDOWN_POINTER[1]);
            context.mouse_button(bite_imgui::MouseButton::Left, frame == 1);
        }
        if scene == Scene::Menu {
            // The dropdown is opened the way a person opens it, by pointing at the bar and
            // pressing. It stays open for the rest of the frames.
            context.mouse_position(MENU_POINTER[0], MENU_POINTER[1]);
            context.mouse_button(bite_imgui::MouseButton::Left, frame == 1);
        }
        data = app::draw_frame(&mut editor, &mut context, logical, scale, 1.0 / 60.0);
    }
    renderer.render(&view, &data, physical.0, physical.1, scale);

    read_back(&renderer, &target, physical, output)?;
    println!("Captured {} to {}", scene.name(), output.display());
    Ok(())
}

/// Copies the rendered texture into a file.
fn read_back(
    renderer: &Renderer,
    texture: &wgpu::Texture,
    size: (u32, u32),
    output: &Path,
) -> Result<(), String> {
    // Buffer rows must be a multiple of two hundred and fifty six bytes.
    let unpadded = size.0 * 4;
    let padded = unpadded.div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: u64::from(padded) * u64::from(size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.1),
            },
        },
        texture.size(),
    );
    renderer.queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| error.to_string())?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

    let mapped = slice.get_mapped_range().map_err(|error| error.to_string())?;
    let mut pixels = Vec::with_capacity(unpadded as usize * size.1 as usize);
    for row in 0..size.1 {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&mapped[start..start + unpadded as usize]);
    }
    drop(mapped);
    buffer.unmap();

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = std::fs::File::create(output).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Captures every scene into a directory.
pub fn capture_all(
    directory: &Path,
    workflow: Option<&Path>,
    size: (u32, u32),
    scale: f32,
) -> Result<Vec<PathBuf>, String> {
    let mut written = Vec::new();
    for scene in Scene::all() {
        let name = if (scale - 1.0).abs() < f32::EPSILON {
            format!("{}.png", scene.name())
        } else {
            format!("{}@{scale}x.png", scene.name())
        };
        let output = directory.join(name);
        capture(scene, &output, workflow, size, scale)?;
        written.push(output);
    }
    Ok(written)
}

/// The colors a capture is expected to contain, as a quick sanity check.
pub fn expected_surface_colors() -> Vec<bite_imgui::Color> {
    vec![theme::GAP_COLOR, theme::PANEL_BG, theme::PANEL_HEADER_BG]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_has_a_stable_name_that_round_trips() {
        for scene in Scene::all() {
            assert_eq!(Scene::from_name(scene.name()), Some(scene));
        }
        assert_eq!(Scene::from_name("nonsense"), None);
    }

    #[test]
    fn the_scene_list_covers_the_canvas_and_every_dialog() {
        let scenes = Scene::all();
        assert!(scenes.contains(&Scene::Seed));
        assert!(scenes.contains(&Scene::CreateMenu));
        assert!(scenes.contains(&Scene::BatchSummary));
        assert_eq!(scenes.len(), 15);
    }
}
