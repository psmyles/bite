//! Phase 8 technical smoke mode plus the Phase 9 functional native GUI.
mod dialogs;
mod renderer;
mod studio;
mod updates;
use bite_core::execution::ImageHost;
use bite_imgui::{Context, Key};
use bite_schema::{BuiltinNodeKind, NodeKind, ParamType, Position, WidgetType};
use renderer::Renderer;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc},
    time::Instant,
};
use studio::Studio;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

type TextPreview = (String, Result<Vec<String>, String>);

struct Demo {
    context: Context,
    text: String,
    value: f32,
    dropped: String,
    links: Vec<(u64, u64, u64)>,
    frames: u64,
    selected: u64,
    selected_nodes: Vec<String>,
    studio: Studio,
    workflow_path: String,
    input_path: String,
    output_path: String,
    runtime_paths: BTreeMap<String, String>,
    runner: Option<RunState>,
    technical_demo: bool,
    import_requested: bool,
    preview_requested: Option<usize>,
    selected_image: usize,
    image_paths: Vec<PathBuf>,
    thumbnails: Vec<u64>,
    preview_texture: Option<u64>,
    active_input: Option<String>,
    input_media: BTreeMap<String, InputMedia>,
    resolved_values: BTreeMap<String, bite_expr::Context>,
    control_down: bool,
    shift_down: bool,
    pending_action: Option<PendingAction>,
    exit_requested: bool,
    creation_position: Option<Position>,
    pending_wire: Option<PendingWire>,
    open_creation_requested: bool,
    node_search: String,
    show_workflow_tools: bool,
    update_receiver: Option<mpsc::Receiver<Result<updates::UpdateInfo, String>>>,
    update_result: Option<Result<updates::UpdateInfo, String>>,
    update_show_all: bool,
    text_preview_receiver: Option<mpsc::Receiver<TextPreview>>,
    text_preview_result: Option<TextPreview>,
}
#[derive(Clone, Default)]
struct InputMedia {
    image_paths: Vec<PathBuf>,
    thumbnails: Vec<u64>,
    selected_image: usize,
    preview_texture: Option<u64>,
}
#[derive(Clone)]
struct PendingWire {
    node: String,
    handle: String,
    output: bool,
}
#[derive(Clone, Copy)]
enum PendingAction {
    Exit,
    New,
    Open,
}
struct RunState {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    events: mpsc::Receiver<RunEvent>,
}
enum RunEvent {
    Progress(usize, usize, String),
    Complete(Result<bite_core::execution::BatchResult, String>),
}

fn stable_id(text: &str) -> u64 {
    let mut value = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        value = (value ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    value.max(1)
}

fn human_label(name: &str) -> String {
    let name = name.trim_start_matches('_');
    let mut label = String::new();
    for (index, character) in name.chars().enumerate() {
        if character == '_' {
            label.push(' ');
        } else {
            if index > 0 && character.is_uppercase() {
                label.push(' ');
            }
            label.push(character);
        }
    }
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label.replace("Cli ", "CLI ")
}

fn sanitize_cli_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                Some(character)
            } else if character.is_ascii_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn upstream_contains_definition(
    graph: &bite_schema::Graph,
    node_id: &str,
    definition_id: &str,
) -> bool {
    let mut pending = vec![node_id];
    let mut visited = BTreeSet::new();
    while let Some(target) = pending.pop() {
        for edge in graph.edges.iter().filter(|edge| edge.target == target) {
            if !visited.insert(edge.source.as_str()) {
                continue;
            }
            if let Some(node) = graph.nodes.iter().find(|node| node.id == edge.source) {
                if node.data.definition_id == definition_id {
                    return true;
                }
                pending.push(&node.id);
            }
        }
    }
    false
}

fn upstream_input_id(graph: &bite_schema::Graph, node_id: &str) -> Option<String> {
    let mut pending = vec![node_id];
    let mut visited = BTreeSet::new();
    while let Some(target) = pending.pop() {
        let node = graph.nodes.iter().find(|node| node.id == target)?;
        if node.kind == NodeKind::Builtin(BuiltinNodeKind::Input) {
            return Some(node.id.clone());
        }
        for edge in graph.edges.iter().filter(|edge| edge.target == target) {
            if visited.insert(edge.source.as_str()) {
                pending.push(&edge.source);
            }
        }
    }
    None
}

fn wire_color(kind: bite_core::graph::WireType) -> [f32; 3] {
    use bite_core::graph::WireType;
    match kind {
        WireType::Image => [0.25, 0.65, 0.95],
        WireType::Mask => [0.95, 0.62, 0.2],
        WireType::Number | WireType::Numeric => [0.35, 0.8, 0.45],
        WireType::Bool => [0.9, 0.32, 0.35],
        WireType::String | WireType::Path => [0.9, 0.75, 0.25],
        WireType::Vector2 | WireType::Vector3 | WireType::Vector4 => [0.55, 0.45, 0.95],
        WireType::Color => [0.95, 0.35, 0.75],
        WireType::Value => [0.7, 0.7, 0.72],
    }
}

fn builtin_options(name: &str) -> Vec<String> {
    let values: &[&str] = match name {
        "outputPath" => &["source", "custom"],
        "overwrite" => &["skip", "overwrite"],
        "separatorType" => &["comma", "tab", "space", "custom"],
        "sortBy" => &["import_order", "name", "name_desc"],
        _ => &[],
    };
    values.iter().map(|value| (*value).into()).collect()
}

fn builtin_labels(name: &str) -> Vec<String> {
    let values: &[&str] = match name {
        "outputPath" => &["Same as source", "Custom folder"],
        "overwrite" => &["Skip existing", "Overwrite"],
        "separatorType" => &["Comma", "Tab", "Space", "Custom…"],
        "sortBy" => &["Import order", "File name (A–Z)", "File name (Z–A)"],
        _ => &[],
    };
    values.iter().map(|value| (*value).into()).collect()
}

fn layout_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("BITE_LAYOUT_PATH") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    // Bump the persisted layout when the source-backed Electron shell changes.
    // This prevents obsolete prototype docks from overriding the new default.
    root.map(|root| root.join("BITE/native-layout-v2.ini"))
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("PNG buffer too large")?];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let source = &buffer[..info.buffer_size()];
    let pixels = match info.color_type {
        png::ColorType::Rgba => source.to_vec(),
        png::ColorType::Rgb => source
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => source
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => source
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => return Err("Unexpected indexed PNG".into()),
    };
    Ok((info.width, info.height, pixels))
}

type UiPorts = Vec<(String, String)>;
fn node_ports(node: &bite_schema::GraphNode, registry: &bite_core::Registry) -> (UiPorts, UiPorts) {
    match node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => {
            (Vec::new(), vec![("out:output".into(), "Image".into())])
        }
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => (
            vec![
                ("in:input".into(), "Image".into()),
                ("folder-in".into(), "Folder".into()),
            ],
            Vec::new(),
        ),
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            let mut inputs = vec![("in:input".into(), "Image".into())];
            if let Some(bite_schema::ParamValue::Structured(
                bite_schema::StructuredParam::TextSlots { slots },
            )) = node.data.params.get("portIds")
            {
                inputs.extend(
                    slots.iter().enumerate().map(|(index, slot)| {
                        (format!("txo:{slot}"), format!("Text {}", index + 1))
                    }),
                );
            }
            (inputs, Vec::new())
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => (
            vec![
                ("in:input".into(), "Image".into()),
                ("param:bgColor".into(), "Background".into()),
            ],
            Vec::new(),
        ),
        _ if node.data.definition_id == "process_as_set" => {
            let suffixes = match node.data.params.get("suffixes") {
                Some(bite_schema::ParamValue::Structured(
                    bite_schema::StructuredParam::SetSuffixes { suffixes },
                )) => suffixes.clone(),
                _ => Vec::new(),
            };
            let mut inputs = vec![
                ("in:input".into(), "Images".into()),
                ("param:prefix".into(), "Prefix".into()),
            ];
            inputs.extend(
                suffixes
                    .iter()
                    .enumerate()
                    .map(|(index, suffix)| (format!("param:suffix_{index}"), suffix.clone())),
            );
            let outputs = suffixes
                .iter()
                .enumerate()
                .map(|(index, suffix)| (format!("out:suffix_{index}"), suffix.clone()))
                .collect();
            (inputs, outputs)
        }
        _ => registry
            .nodes
            .get(&node.data.definition_id)
            .map(|definition| {
                let input_definitions = if node.data.inputs.is_empty() {
                    &definition.definition.inputs
                } else {
                    &node.data.inputs
                };
                let output_definitions = if node.data.outputs.is_empty() {
                    &definition.definition.outputs
                } else {
                    &node.data.outputs
                };
                let mut inputs: UiPorts = input_definitions
                    .iter()
                    .map(|port| (format!("in:{}", port.name), port.label.clone()))
                    .collect();
                let mut outputs: UiPorts = output_definitions
                    .iter()
                    .map(|port| (format!("out:{}", port.name), port.label.clone()))
                    .collect();
                let mut context =
                    bite_expr::definition::default_context(&definition.definition.params);
                for (name, value) in &node.data.params {
                    if let Some(value) = bite_expr::definition::to_value(value) {
                        context.insert(name.clone(), value);
                    }
                }
                for parameter in &definition.definition.params {
                    let visible = definition
                        .visible
                        .get(&parameter.name)
                        .map(|expression| {
                            expression
                                .evaluate(&context)
                                .map(|value| value.truthy())
                                .unwrap_or(true)
                        })
                        .unwrap_or(true);
                    if parameter.no_port || !visible {
                        continue;
                    }
                    let port = (format!("param:{}", parameter.name), parameter.label.clone());
                    if parameter.readonly {
                        outputs.push(port);
                    } else {
                        inputs.push(port);
                    }
                }
                (inputs, outputs)
            })
            .unwrap_or_default(),
    }
}

fn candidate_node(studio: &Studio, kind: NodeKind, definition_id: &str) -> bite_schema::GraphNode {
    let params = studio
        .registry
        .nodes
        .get(definition_id)
        .map(|compiled| {
            compiled
                .definition
                .params
                .iter()
                .filter_map(|parameter| {
                    parameter
                        .default
                        .clone()
                        .map(|value| (parameter.name.clone(), value))
                })
                .collect()
        })
        .unwrap_or_else(|| match kind {
            NodeKind::Builtin(BuiltinNodeKind::TextOutput) => BTreeMap::from([(
                "portIds".into(),
                bite_schema::ParamValue::Structured(bite_schema::StructuredParam::TextSlots {
                    slots: vec!["0".into()],
                }),
            )]),
            _ => BTreeMap::new(),
        });
    bite_schema::GraphNode {
        id: "candidate".into(),
        kind,
        position: Position { x: 0.0, y: 0.0 },
        parent_id: None,
        extent: None,
        width: None,
        height: None,
        data: bite_schema::NodeData {
            label: String::new(),
            definition_id: definition_id.into(),
            params,
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
    }
}

fn accepts_pending_wire(
    studio: &Studio,
    pending: &PendingWire,
    candidate: &bite_schema::GraphNode,
) -> bool {
    let Some(existing) = studio
        .workflow
        .graph
        .nodes
        .iter()
        .find(|node| node.id == pending.node)
    else {
        return false;
    };
    let Ok(existing_type) =
        bite_core::graph::handle_type(existing, &pending.handle, pending.output, &studio.registry)
    else {
        return false;
    };
    let (inputs, outputs) = node_ports(candidate, &studio.registry);
    let handles = if pending.output { inputs } else { outputs };
    handles.into_iter().any(|(handle, _)| {
        bite_core::graph::handle_type(candidate, &handle, !pending.output, &studio.registry)
            .is_ok_and(|candidate_type| bite_core::graph::compatible(existing_type, candidate_type))
    })
}

fn candidate_allowed(
    studio: &Studio,
    pending: &Option<PendingWire>,
    kind: NodeKind,
    definition_id: &str,
) -> bool {
    pending.as_ref().is_none_or(|pending| {
        accepts_pending_wire(
            studio,
            pending,
            &candidate_node(studio, kind, definition_id),
        )
    })
}

fn copy_selection(studio: &mut Studio, selected: &[String]) {
    if studio.copy_selection(selected) {
        if let Some(fragment) = studio.clipboard_json() {
            if let Err(error) = dialogs::write_clipboard(&fragment) {
                studio.status = format!("Copied in app; system clipboard failed: {error}");
            }
        }
    }
}

fn paste_selection(studio: &mut Studio) -> Vec<String> {
    match dialogs::read_clipboard() {
        Ok(Some(fragment)) => {
            studio.import_clipboard_json(&fragment);
        }
        Ok(None) => {}
        Err(error) => studio.status = format!("System clipboard failed: {error}"),
    }
    studio.paste()
}
impl Demo {
    fn new(renderer: &mut Renderer, scale: f32, technical_demo: bool) -> Result<Self, String> {
        let layout = layout_path();
        if let Some(parent) = layout.as_deref().and_then(std::path::Path::parent) {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut context = Context::with_scale_and_ini(scale, layout.as_deref())?;
        let (w, h, pixels) = context.font_atlas();
        renderer.texture(1, w, h, &pixels);
        context.set_font_texture(1);
        let pixels: Vec<u8> = (0..256)
            .flat_map(|y| {
                (0..256).flat_map(move |x| {
                    [
                        x as u8,
                        y as u8,
                        if (x / 16 + y / 16) % 2 == 0 { 200 } else { 70 },
                        255,
                    ]
                })
            })
            .collect();
        renderer.texture(2, 256, 256, &pixels);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root)?;
        let initial = std::env::args()
            .nth(1)
            .filter(|value| !value.starts_with("--"));
        let mut studio = if let Some(path) = initial.as_deref() {
            Studio::open(registry.clone(), PathBuf::from(path).as_path())
                .unwrap_or_else(|_| Studio::blank(registry))
        } else {
            Studio::blank(registry)
        };
        if studio.workflow.graph.nodes.is_empty() && !technical_demo {
            studio.add_input(Position { x: 0.0, y: 100.0 });
            studio.add_output(Position { x: 700.0, y: 100.0 });
            studio.mark_clean();
        }
        let workflow_path = initial.unwrap_or_else(|| "workflow.bite".into());
        let (update_send, update_receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = update_send.send(updates::check());
        });
        Ok(Self {
            context,
            text: "Multiline input\nType here to test input and IME.".into(),
            value: 1.,
            dropped: "Drop files into this window".into(),
            links: (0..119)
                .map(|i| (10000 + i, i * 10 + 3, (i + 1) * 10 + 2))
                .collect(),
            frames: 0,
            selected: 0,
            selected_nodes: Vec::new(),
            studio,
            workflow_path,
            input_path: root.join("test_images").to_string_lossy().into_owned(),
            output_path: root
                .join("test-workflows/out/native-gui")
                .to_string_lossy()
                .into_owned(),
            runtime_paths: BTreeMap::new(),
            runner: None,
            technical_demo,
            import_requested: false,
            preview_requested: None,
            selected_image: 0,
            image_paths: Vec::new(),
            thumbnails: Vec::new(),
            preview_texture: None,
            active_input: None,
            input_media: BTreeMap::new(),
            resolved_values: BTreeMap::new(),
            control_down: false,
            shift_down: false,
            pending_action: None,
            exit_requested: false,
            creation_position: None,
            pending_wire: None,
            open_creation_requested: false,
            node_search: String::new(),
            show_workflow_tools: false,
            update_receiver: Some(update_receive),
            update_result: None,
            update_show_all: false,
            text_preview_receiver: None,
            text_preview_result: None,
        })
    }

    fn activate_input(&mut self, node_id: &str) {
        if self.active_input.as_deref() == Some(node_id) {
            return;
        }
        if let Some(previous) = self.active_input.take() {
            self.input_media.insert(
                previous,
                InputMedia {
                    image_paths: std::mem::take(&mut self.image_paths),
                    thumbnails: std::mem::take(&mut self.thumbnails),
                    selected_image: self.selected_image,
                    preview_texture: self.preview_texture.take(),
                },
            );
        }
        let media = self.input_media.remove(node_id).unwrap_or_default();
        self.image_paths = media.image_paths;
        self.thumbnails = media.thumbnails;
        self.selected_image = media.selected_image;
        self.preview_texture = media.preview_texture;
        self.active_input = Some(node_id.to_owned());
        self.text_preview_receiver = None;
        self.text_preview_result = None;
    }

    fn clear_media(&mut self) {
        self.active_input = None;
        self.input_media.clear();
        self.image_paths.clear();
        self.thumbnails.clear();
        self.selected_image = 0;
        self.preview_texture = None;
        self.resolved_values.clear();
        self.text_preview_receiver = None;
        self.text_preview_result = None;
    }

    fn import_images(&mut self, renderer: &mut Renderer) -> Result<(), String> {
        let selected_input = self
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Builtin(BuiltinNodeKind::Input)
                    && self.active_input.as_deref() == Some(node.id.as_str())
            })
            .or_else(|| {
                self.studio.workflow.graph.nodes.iter().find(|node| {
                    node.kind == NodeKind::Builtin(BuiltinNodeKind::Input)
                        && stable_id(&format!("node:{}", node.id)) == self.selected
                })
            })
            .or_else(|| {
                self.studio
                    .workflow
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            })
            .map(|node| {
                let thumbnail_size = node
                    .data
                    .params
                    .get("thumbnailSize")
                    .and_then(|value| match value {
                        bite_schema::ParamValue::Int(value) => u32::try_from(*value).ok(),
                        _ => None,
                    })
                    .unwrap_or(256)
                    .clamp(64, 2048);
                (node.id.clone(), thumbnail_size)
            })
            .ok_or("Workflow has no Input node")?;
        self.activate_input(&selected_input.0);
        self.text_preview_receiver = None;
        self.text_preview_result = None;
        let root = self
            .runtime_paths
            .get(&selected_input.0)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&self.input_path));
        let thumbnail_size = selected_input.1;
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let paths = bite_imagemagick::import::scan_folder(&root, false, 8, &cancelled)?;
        let cache_dir = std::env::temp_dir().join("bite-native-thumbnails");
        let mut cache = bite_imagemagick::import::ThumbnailCache::new(cache_dir.clone());
        cache.jobs = 8;
        let mut host = bite_imagemagick::Magick::discover(Arc::new(cancelled));
        let images = cache.load_batch(&mut host, &paths, thumbnail_size)?;
        self.thumbnails.clear();
        self.image_paths = paths.iter().take(32).cloned().collect();
        for (index, image) in images.iter().take(32).enumerate() {
            let png_path = cache_dir.join(format!("gui-{}-{index}.png", std::process::id()));
            host.run(&[
                image.thumbnail.to_string_lossy().into_owned(),
                png_path.to_string_lossy().into_owned(),
            ])?;
            let bytes = std::fs::read(&png_path).map_err(|error| error.to_string())?;
            let (width, height, pixels) = decode_png(&bytes)?;
            let texture = stable_id(&format!("thumbnail:{}:{index}", selected_input.0));
            renderer.texture(texture, width, height, &pixels);
            self.thumbnails.push(texture);
        }
        if !self.image_paths.is_empty() {
            self.refresh_preview(renderer, 0)?;
        }
        self.studio.status = format!(
            "Imported {} images at {thumbnail_size}px thumbnail size",
            images.len()
        );
        Ok(())
    }

    fn refresh_preview(&mut self, renderer: &mut Renderer, index: usize) -> Result<(), String> {
        let image = self
            .image_paths
            .get(index)
            .ok_or_else(|| format!("Image {index} is no longer in the filmstrip"))?;
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut host = bite_imagemagick::Magick::discover(cancelled);
        let selected_node = self
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| stable_id(&format!("node:{}", node.id)) == self.selected);
        let explicit_source = selected_node.and_then(|node| {
            if node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput) {
                self.studio
                    .workflow
                    .graph
                    .edges
                    .iter()
                    .find(|edge| edge.target == node.id && edge.target_handle == "in:input")
                    .map(|edge| (edge.source.clone(), edge.source_handle.clone()))
            } else {
                node_ports(node, &self.studio.registry)
                    .1
                    .into_iter()
                    .find(|(handle, _)| {
                        matches!(
                            bite_core::graph::handle_type(
                                node,
                                handle,
                                true,
                                &self.studio.registry
                            ),
                            Ok(bite_core::graph::WireType::Image)
                        )
                    })
                    .map(|(handle, _)| (node.id.clone(), handle))
            }
        });
        let preview = bite_core::preview::render(
            &self.studio.workflow.graph,
            &self.studio.registry,
            &mut host,
            image,
            explicit_source
                .as_ref()
                .map(|(node, handle)| (node.as_str(), handle.as_str())),
        )?;
        let (width, height, pixels) = decode_png(&preview.png)?;
        let texture = stable_id(&format!(
            "preview:{}:{index}",
            self.active_input.as_deref().unwrap_or("default")
        ));
        renderer.texture(texture, width, height, &pixels);
        self.preview_texture = Some(texture);
        self.selected_image = index;
        self.resolved_values = preview.resolved_values;
        self.studio.status = format!("Previewing {}", image.display());
        Ok(())
    }
    fn draw(&mut self, width: u32, height: u32, scale: f32, delta: f32) -> bite_imgui::DrawData {
        let selected_input = self
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| stable_id(&format!("node:{}", node.id)) == self.selected)
            .and_then(|node| upstream_input_id(&self.studio.workflow.graph, &node.id));
        if let Some(input) = selected_input {
            self.activate_input(&input);
        }
        let Self {
            context,
            text,
            value,
            dropped,
            links,
            frames,
            selected,
            selected_nodes,
            studio,
            workflow_path,
            input_path,
            output_path,
            runtime_paths,
            runner,
            technical_demo,
            import_requested,
            preview_requested,
            selected_image,
            image_paths,
            thumbnails,
            preview_texture,
            active_input,
            input_media,
            resolved_values,
            control_down: _,
            shift_down: _,
            pending_action,
            exit_requested,
            creation_position,
            pending_wire,
            open_creation_requested,
            node_search,
            show_workflow_tools,
            update_receiver,
            update_result,
            update_show_all,
            text_preview_receiver,
            text_preview_result,
        } = self;
        let mut completed = false;
        if let Some(active) = runner.as_ref() {
            while let Ok(event) = active.events.try_recv() {
                match event {
                    RunEvent::Progress(done, total, file) => {
                        studio.status = format!("Running {done}/{total}: {file}");
                    }
                    RunEvent::Complete(result) => {
                        studio.status = match result {
                            Ok(result) => format!(
                                "Run complete: {} processed, {} skipped, {} failed",
                                result.processed, result.skipped, result.failed
                            ),
                            Err(error) => format!("Run failed: {error}"),
                        };
                        completed = true;
                    }
                }
            }
        }
        if completed {
            *runner = None;
        }
        if let Some(receiver) = update_receiver.as_ref() {
            if let Ok(result) = receiver.try_recv() {
                if *update_show_all
                    || result
                        .as_ref()
                        .is_ok_and(|information| information.available)
                {
                    *update_result = Some(result);
                }
                *update_receiver = None;
                *update_show_all = false;
            }
        }
        if let Some(receiver) = text_preview_receiver.as_ref() {
            if let Ok(result) = receiver.try_recv() {
                *text_preview_result = Some(result);
                *text_preview_receiver = None;
            }
        }
        let mut frame = context.frame(
            width as f32 / scale,
            height as f32 / scale,
            scale,
            delta.clamp(0.001, 0.1),
        );
        let mut ui = frame.ui();
        let mut action = None;
        ui.main_menu_bar(|ui| {
            ui.text("Bite");
            ui.menu("File", |ui| {
                if ui.menu_item("New", "Ctrl+N", false, true) {
                    action = Some(PendingAction::New);
                }
                if ui.menu_item("Open...", "Ctrl+O", false, true) {
                    match dialogs::open_workflow() {
                        Ok(Some(path)) => {
                            *workflow_path = path.to_string_lossy().into_owned();
                            action = Some(PendingAction::Open);
                        }
                        Ok(None) => {}
                        Err(error) => studio.status = format!("Open dialog failed: {error}"),
                    }
                }
                if ui.menu_item("Save", "Ctrl+S", false, true) {
                    if let Err(error) = studio.save(PathBuf::from(&*workflow_path).as_path()) {
                        studio.status = format!("Save failed: {error}");
                    }
                }
                if ui.menu_item("Workflow settings...", "", *show_workflow_tools, true) {
                    *show_workflow_tools = !*show_workflow_tools;
                }
            });
            ui.menu("Edit", |ui| {
                if ui.menu_item("Undo", "Ctrl+Z", false, studio.can_undo()) && studio.undo() {
                    *frames = 0;
                }
                if ui.menu_item("Redo", "Ctrl+Y", false, studio.can_redo()) && studio.redo() {
                    *frames = 0;
                }
                if ui.menu_item("Duplicate", "Ctrl+D", false, !selected_nodes.is_empty()) {
                    let pasted = studio.duplicate_selection(selected_nodes);
                    if let Some(id) = pasted.first() {
                        *selected = stable_id(&format!("node:{id}"));
                        *frames = 0;
                    }
                }
                if ui.menu_item("Delete", "Del", false, !selected_nodes.is_empty()) {
                    studio.delete_selection(selected_nodes);
                    *selected = 0;
                    *frames = 0;
                }
            });
            ui.menu("View", |ui| {
                if ui.menu_item("Frame workflow", "Ctrl+0", false, true) {
                    *frames = 1;
                }
            });
            ui.menu("Help", |ui| {
                if ui.menu_item("Check for Updates", "", false, update_receiver.is_none()) {
                    let (send, receive) = mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = send.send(updates::check());
                    });
                    *update_show_all = true;
                    *update_receiver = Some(receive);
                    *update_result = None;
                    studio.status = "Checking for updates…".into();
                }
            });
        });
        ui.dockspace();
        if *show_workflow_tools {
            ui.window("Workflow settings", |ui| {
                ui.text("Workflow file");
                ui.next_item_full_width();
                ui.input_text("##workflow-path", workflow_path);
                if ui.button("New") {
                    if studio.dirty {
                        *pending_action = Some(PendingAction::New);
                    } else {
                        action = Some(PendingAction::New);
                    }
                }
                ui.same_line();
                if ui.button("Open") {
                    match dialogs::open_workflow() {
                        Ok(Some(path)) => {
                            *workflow_path = path.to_string_lossy().into_owned();
                            if studio.dirty {
                                *pending_action = Some(PendingAction::Open);
                            } else {
                                action = Some(PendingAction::Open);
                            }
                        }
                        Ok(None) => {}
                        Err(error) => studio.status = format!("Open dialog failed: {error}"),
                    }
                }
                ui.same_line();
                if ui.button("Save") {
                    if let Err(error) = studio.save(PathBuf::from(&*workflow_path).as_path()) {
                        studio.status = format!("Save failed: {error}");
                    }
                }
                ui.same_line();
                if ui.button("Save As") {
                    match dialogs::save_file(
                        PathBuf::from(&*workflow_path).as_path(),
                        "bite",
                        "BITE workflow",
                    ) {
                        Ok(Some(path)) => match studio.save(&path) {
                            Ok(()) => *workflow_path = path.to_string_lossy().into_owned(),
                            Err(error) => studio.status = format!("Save failed: {error}"),
                        },
                        Ok(None) => {}
                        Err(error) => studio.status = format!("Save dialog failed: {error}"),
                    }
                }
                let undo_label = if studio.can_undo() {
                    "Undo"
                } else {
                    "Undo (empty)"
                };
                if ui.button(undo_label) && studio.undo() {
                    *frames = 0;
                }
                ui.same_line();
                let redo_label = if studio.can_redo() {
                    "Redo"
                } else {
                    "Redo (empty)"
                };
                if ui.button(redo_label) && studio.redo() {
                    *frames = 0;
                }
                if ui.button("Copy") {
                    copy_selection(studio, selected_nodes);
                }
                ui.same_line();
                if ui.button("Paste") {
                    let pasted = paste_selection(studio);
                    if let Some(id) = pasted.first() {
                        *selected = stable_id(&format!("node:{id}"));
                        *frames = 0;
                    }
                }
                ui.same_line();
                if ui.button("Duplicate") {
                    let pasted = studio.duplicate_selection(selected_nodes);
                    if let Some(id) = pasted.first() {
                        *selected = stable_id(&format!("node:{id}"));
                        *frames = 0;
                    }
                }
                ui.same_line();
                if ui.button("Delete") {
                    let deleting_active = active_input
                        .as_ref()
                        .is_some_and(|id| selected_nodes.contains(id));
                    let deleting = selected_nodes.clone();
                    if studio.delete_selection(selected_nodes) {
                        for id in &deleting {
                            runtime_paths.remove(id);
                            input_media.remove(id);
                        }
                        if deleting_active {
                            *active_input = None;
                            image_paths.clear();
                            thumbnails.clear();
                            *selected_image = 0;
                            *preview_texture = None;
                            resolved_values.clear();
                            *text_preview_receiver = None;
                            *text_preview_result = None;
                        }
                        *selected = 0;
                        selected_nodes.clear();
                        *frames = 0;
                    }
                }
                if ui.button("Group") {
                    if let Some(id) = studio.group_selection(selected_nodes) {
                        *selected = stable_id(&format!("node:{id}"));
                        *frames = 0;
                    }
                }
                ui.same_line();
                if ui.button("Ungroup") && studio.ungroup_selection(selected_nodes) {
                    *selected = 0;
                    *frames = 0;
                }
                ui.text("Export CLI script");
                for (index, (label, extension, shell)) in [
                    (
                        "PowerShell",
                        "ps1",
                        bite_core::cli_export::Shell::PowerShell,
                    ),
                    ("Bash", "sh", bite_core::cli_export::Shell::Bash),
                    ("CMD", "bat", bite_core::cli_export::Shell::Cmd),
                ]
                .into_iter()
                .enumerate()
                {
                    if ui.button(label) {
                        let default = PathBuf::from(&*workflow_path).with_extension(extension);
                        match dialogs::save_file(&default, extension, label) {
                            Ok(Some(path)) => {
                                if let Err(error) = studio.export_cli(&path, shell) {
                                    studio.status = format!("Export failed: {error}");
                                }
                            }
                            Ok(None) => {}
                            Err(error) => studio.status = format!("Export dialog failed: {error}"),
                        }
                    }
                    if index < 2 {
                        ui.same_line();
                    }
                }
                if ui.button("Reload definitions") {
                    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
                    if let Err(error) = studio.reload_registry(&root) {
                        studio.status = format!("Definition reload failed: {error}");
                    }
                }
                if update_receiver.is_none() && ui.button("Check for updates") {
                    let (send, receive) = mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = send.send(updates::check());
                    });
                    *update_receiver = Some(receive);
                    *update_result = None;
                    *update_show_all = true;
                    studio.status = "Checking for updates…".into();
                } else if update_receiver.is_some() {
                    ui.text("Checking for updates…");
                }
                ui.text("Input folder");
                ui.next_item_full_width();
                ui.input_text("##input-folder", input_path);
                if ui.button("Browse input") {
                    match dialogs::select_folder() {
                        Ok(Some(path)) => *input_path = path.to_string_lossy().into_owned(),
                        Ok(None) => {}
                        Err(error) => studio.status = format!("Folder dialog failed: {error}"),
                    }
                }
                ui.text("Output folder");
                ui.next_item_full_width();
                ui.input_text("##output-folder", output_path);
                if ui.button("Browse output") {
                    match dialogs::select_folder() {
                        Ok(Some(path)) => *output_path = path.to_string_lossy().into_owned(),
                        Ok(None) => {}
                        Err(error) => studio.status = format!("Folder dialog failed: {error}"),
                    }
                }
                if ui.button("Import images") {
                    *import_requested = true;
                    studio.status = "Importing images…".into();
                }
                if runner.is_none() && ui.button("Run") {
                    let graph = studio.workflow.graph.clone();
                    let registry = studio.registry.clone();
                    let options = studio.run_options_with_overrides(
                        PathBuf::from(&*input_path).as_path(),
                        PathBuf::from(&*output_path).as_path(),
                        runtime_paths,
                    );
                    let cancelled = options.cancelled.clone();
                    let (send, receive) = mpsc::channel();
                    std::thread::spawn(move || {
                        let mut host =
                            bite_imagemagick::Magick::discover(options.cancelled.clone());
                        let progress = send.clone();
                        let result = bite_core::execution::run_workflow(
                            &graph,
                            &registry,
                            &mut host,
                            &options,
                            &mut |done, total, file| {
                                let _ = progress.send(RunEvent::Progress(
                                    done,
                                    total,
                                    file.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                ));
                            },
                        );
                        let _ = send.send(RunEvent::Complete(result));
                    });
                    *runner = Some(RunState {
                        cancelled,
                        events: receive,
                    });
                    studio.status = "Running workflow…".into();
                } else if let Some(active) = runner.as_ref() {
                    if ui.button("Cancel") {
                        active.cancelled.store(true, Ordering::Relaxed);
                        studio.status = "Cancelling…".into();
                    }
                }
                ui.text(&studio.status);
            });
            let mut dismiss_update = false;
            let mut copy_update_url = None;
            if let Some(result) = update_result.as_ref() {
                ui.window("BITE update", |ui| {
                    match result {
                        Ok(update) if update.available => {
                            ui.text(&format!("Update available: {}", update.version));
                            for line in update.body.lines().take(20) {
                                ui.text(line);
                            }
                            ui.text(&update.url);
                            if ui.button("Copy release URL") {
                                copy_update_url = Some(update.url.clone());
                            }
                        }
                        Ok(update) => {
                            ui.text(&format!(
                                "BITE is up to date (latest release: {})",
                                update.version
                            ));
                        }
                        Err(error) => ui.text(&format!("Update check failed: {error}")),
                    }
                    if ui.button("Dismiss") {
                        dismiss_update = true;
                    }
                });
            }
            if let Some(url) = copy_update_url {
                studio.status = match dialogs::write_clipboard(&url) {
                    Ok(()) => "Copied the release URL".into(),
                    Err(error) => format!("Could not copy the release URL: {error}"),
                };
            }
            if dismiss_update {
                *update_result = None;
            }
            if let Some(pending) = *pending_action {
                ui.window("Unsaved changes", |ui| {
                    ui.text("Save the current workflow before continuing?");
                    if ui.button("Save and continue") {
                        match studio.save(PathBuf::from(&*workflow_path).as_path()) {
                            Ok(()) => action = Some(pending),
                            Err(error) => studio.status = format!("Save failed: {error}"),
                        }
                        *pending_action = None;
                    }
                    ui.same_line();
                    if ui.button("Discard") {
                        action = Some(pending);
                        *pending_action = None;
                    }
                    ui.same_line();
                    if ui.button("Cancel") {
                        *pending_action = None;
                    }
                });
            }
            if let Some(action) = action {
                match action {
                    PendingAction::Exit => *exit_requested = true,
                    PendingAction::New => {
                        *studio = Studio::blank(studio.registry.clone());
                        studio.add_input(Position { x: 0.0, y: 100.0 });
                        studio.add_output(Position { x: 700.0, y: 100.0 });
                        studio.mark_clean();
                        *workflow_path = "workflow.bite".into();
                        runtime_paths.clear();
                        *active_input = None;
                        input_media.clear();
                        image_paths.clear();
                        thumbnails.clear();
                        *selected_image = 0;
                        *preview_texture = None;
                        resolved_values.clear();
                        *text_preview_receiver = None;
                        *text_preview_result = None;
                        *selected = 0;
                        selected_nodes.clear();
                        *frames = 0;
                    }
                    PendingAction::Open => match Studio::open(
                        studio.registry.clone(),
                        PathBuf::from(&*workflow_path).as_path(),
                    ) {
                        Ok(opened) => {
                            *studio = opened;
                            *selected = 0;
                            selected_nodes.clear();
                            runtime_paths.clear();
                            *active_input = None;
                            input_media.clear();
                            image_paths.clear();
                            thumbnails.clear();
                            *selected_image = 0;
                            *preview_texture = None;
                            resolved_values.clear();
                            *text_preview_receiver = None;
                            *text_preview_result = None;
                            *frames = 0;
                        }
                        Err(error) => studio.status = format!("Open failed: {error}"),
                    },
                }
            }
        }
        ui.window("Library", |ui| {
            if *technical_demo {
                ui.text("Ten fake node types");
                for name in [
                    "Input", "Resize", "Color", "Mask", "Math", "Gate", "Merge", "Text", "Format",
                    "Output",
                ] {
                    ui.text(name);
                }
            } else {
                ui.next_item_full_width();
                ui.input_text("##library-search", node_search);
                ui.text("Workflow");
                if ui.button("Input") {
                    studio.add_input(Position { x: 0.0, y: 0.0 });
                }
                if ui.button("Image Output") {
                    studio.add_output(Position { x: 600.0, y: 0.0 });
                }
                if ui.button("Text Output") {
                    studio.add_text_output(Position { x: 600.0, y: 120.0 });
                }
                if ui.button("Flipbook Output") {
                    studio.add_flipbook_output(Position { x: 600.0, y: 240.0 });
                }
                if ui.button("Comment") {
                    studio.add_comment(Position { x: 250.0, y: 250.0 });
                }
                let query = node_search.trim().to_lowercase();
                let mut definitions: Vec<_> = studio
                    .registry
                    .nodes
                    .values()
                    .filter(|definition| {
                        query.is_empty()
                            || definition.definition.label.to_lowercase().contains(&query)
                            || definition
                                .definition
                                .category
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|definition| {
                        (
                            definition.definition.category.clone(),
                            definition.definition.id.clone(),
                            definition.definition.label.clone(),
                        )
                    })
                    .collect();
                definitions.sort();
                let mut category = String::new();
                for (next_category, id, label) in definitions {
                    if category != next_category {
                        category = next_category;
                        ui.text(&category);
                    }
                    if ui.button(&label) {
                        if let Err(error) =
                            studio.add_processing(&id, Position { x: 300.0, y: 200.0 })
                        {
                            studio.status = error;
                        }
                    }
                }
            }
        });
        ui.window("Canvas", |ui| {
            let mut open_create = false;
            ui.editor("Prototype", |ui| {
                if *technical_demo {
                    for i in 0..120u64 {
                        let id = i * 10 + 1;
                        if *frames == 0 {
                            ui.set_position(id, [(i % 12) as f32 * 200., (i / 12) as f32 * 110.]);
                        }
                        ui.node(id, |ui| {
                            ui.text(&format!(
                                "{} {}",
                                [
                                    "Input", "Resize", "Color", "Mask", "Math", "Gate", "Merge",
                                    "Text", "Format", "Output"
                                ][(i % 10) as usize],
                                i + 1
                            ));
                            ui.pin(i * 10 + 2, false, "In");
                            ui.same_line();
                            ui.pin(i * 10 + 3, true, "Out");
                        });
                        if ui.selected(id) {
                            *selected = id;
                        }
                    }
                    ui.node(5000, |ui| {
                        ui.text("Comment / group");
                        ui.group(360., 180.);
                    });
                    for (id, a, b) in links.iter() {
                        ui.link(
                            *id,
                            *a,
                            *b,
                            if id % 2 == 0 {
                                [0.3, 0.75, 0.95]
                            } else {
                                [0.9, 0.6, 0.2]
                            },
                        );
                    }
                    if let Some((a, b)) = ui.new_link() {
                        links.push((20000 + links.len() as u64, a, b));
                    }
                    if let Some(id) = ui.deleted_link() {
                        links.retain(|(link, _, _)| *link != id);
                    }
                    if ui.background_menu() {
                        *dropped = "Background context menu requested".into();
                    }
                    if *frames == 5 {
                        ui.navigate();
                    }
                } else {
                    if ui.dragging_selection() {
                        studio.begin_transaction();
                    } else {
                        studio.finish_transaction("Moved node selection");
                    }
                    let nodes = studio.workflow.graph.nodes.clone();
                    let edges = studio.workflow.graph.edges.clone();
                    let group_positions: BTreeMap<_, _> = nodes
                        .iter()
                        .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Group))
                        .map(|node| (node.id.clone(), node.position.clone()))
                        .collect();
                    let mut pins: BTreeMap<u64, (String, String, bool)> = BTreeMap::new();
                    let mut positions = Vec::new();
                    let mut inline_changes = Vec::new();
                    let mut current_selection = Vec::new();
                    for node in &nodes {
                        let node_id = stable_id(&format!("node:{}", node.id));
                        let display_position = node
                            .parent_id
                            .as_ref()
                            .and_then(|parent| group_positions.get(parent))
                            .map(|parent| {
                                [
                                    (parent.x + node.position.x) as f32,
                                    (parent.y + node.position.y) as f32,
                                ]
                            })
                            .unwrap_or([node.position.x as f32, node.position.y as f32]);
                        if *frames < 2 {
                            ui.set_position(node_id, display_position);
                        }
                        let (inputs, outputs) = node_ports(node, &studio.registry);
                        ui.node(node_id, |ui| {
                            let header = match node.kind {
                                NodeKind::Builtin(BuiltinNodeKind::Input) => [0.20, 0.42, 0.25],
                                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                                    [0.48, 0.31, 0.10]
                                }
                                NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                                    [0.12, 0.31, 0.50]
                                }
                                NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
                                    [0.35, 0.20, 0.50]
                                }
                                _ => [0.24, 0.24, 0.24],
                            };
                            ui.node_header(&node.data.label, header);
                            if matches!(
                                node.kind,
                                NodeKind::Builtin(
                                    BuiltinNodeKind::Group | BuiltinNodeKind::Comment
                                )
                            ) {
                                ui.group(
                                    node.width.unwrap_or(320.0) as f32,
                                    node.height.unwrap_or(220.0) as f32,
                                );
                            }
                            if node.kind == NodeKind::Builtin(BuiltinNodeKind::Comment) {
                                if let Some(bite_schema::ParamValue::String(body)) =
                                    node.data.params.get("body")
                                {
                                    let mut edited = body.clone();
                                    ui.next_item_full_width();
                                    if ui.input_text(
                                        &format!("##comment-body-{}", node.id),
                                        &mut edited,
                                    ) {
                                        inline_changes.push((node.id.clone(), edited));
                                    }
                                }
                            }
                            for (handle, label) in &inputs {
                                let pin = stable_id(&format!("pin:{}:{handle}", node.id));
                                pins.insert(pin, (node.id.clone(), handle.clone(), false));
                                let color = bite_core::graph::handle_type(
                                    node,
                                    handle,
                                    false,
                                    &studio.registry,
                                )
                                .map(wire_color)
                                .unwrap_or([1.0, 1.0, 1.0]);
                                ui.typed_pin(pin, false, label, color);
                            }
                            for (handle, label) in &outputs {
                                let pin = stable_id(&format!("pin:{}:{handle}", node.id));
                                pins.insert(pin, (node.id.clone(), handle.clone(), true));
                                let color = bite_core::graph::handle_type(
                                    node,
                                    handle,
                                    true,
                                    &studio.registry,
                                )
                                .map(wire_color)
                                .unwrap_or([1.0, 1.0, 1.0]);
                                ui.typed_pin(pin, true, label, color);
                            }
                        });
                        if ui.selected(node_id) {
                            *selected = node_id;
                            current_selection.push(node.id.clone());
                        }
                        let size = matches!(
                            node.kind,
                            NodeKind::Builtin(BuiltinNodeKind::Group | BuiltinNodeKind::Comment)
                        )
                        .then(|| ui.node_size(node_id));
                        positions.push((node.id.clone(), ui.position(node_id), size));
                    }
                    for edge in &edges {
                        let source =
                            stable_id(&format!("pin:{}:{}", edge.source, edge.source_handle));
                        let target =
                            stable_id(&format!("pin:{}:{}", edge.target, edge.target_handle));
                        let color = nodes
                            .iter()
                            .find(|node| node.id == edge.source)
                            .and_then(|node| {
                                bite_core::graph::handle_type(
                                    node,
                                    &edge.source_handle,
                                    true,
                                    &studio.registry,
                                )
                                .ok()
                            })
                            .map(wire_color)
                            .unwrap_or([0.3, 0.65, 0.9]);
                        ui.link(
                            stable_id(&format!("link:{}", edge.id)),
                            source,
                            target,
                            color,
                        );
                    }
                    if let Some((a, b)) = ui.new_link() {
                        let pair = pins.get(&a).zip(pins.get(&b));
                        if let Some((left, right)) = pair {
                            let (source, target) = if left.2 && !right.2 {
                                (left, right)
                            } else {
                                (right, left)
                            };
                            match studio.connect(&source.0, &source.1, &target.0, &target.1) {
                                Ok(_) => studio.status = "Connected nodes".into(),
                                Err(error) => {
                                    studio.status = format!("Connection rejected: {error}")
                                }
                            }
                        }
                    }
                    if let Some(pin) = ui.new_node() {
                        if let Some((node, handle, output)) = pins.get(&pin) {
                            *pending_wire = Some(PendingWire {
                                node: node.clone(),
                                handle: handle.clone(),
                                output: *output,
                            });
                            let position = ui.canvas_mouse_position();
                            *creation_position = Some(Position {
                                x: f64::from(position[0]),
                                y: f64::from(position[1]),
                            });
                            open_create = true;
                        }
                    }
                    if let Some(link) = ui.deleted_link() {
                        if let Some(edge) = studio
                            .workflow
                            .graph
                            .edges
                            .iter()
                            .find(|edge| stable_id(&format!("link:{}", edge.id)) == link)
                            .cloned()
                        {
                            studio.delete_edge(&edge.id);
                        }
                    }
                    let new_positions: BTreeMap<_, _> = positions
                        .iter()
                        .map(|(id, position, _)| (id.clone(), *position))
                        .collect();
                    for (id, position, size) in positions {
                        if let Some(node) = studio
                            .workflow
                            .graph
                            .nodes
                            .iter_mut()
                            .find(|node| node.id == id)
                        {
                            if *frames > 1 {
                                let position = node
                                    .parent_id
                                    .as_ref()
                                    .and_then(|parent| new_positions.get(parent))
                                    .map(|parent| {
                                        [position[0] - parent[0], position[1] - parent[1]]
                                    })
                                    .unwrap_or(position);
                                let changed = node.position.x != f64::from(position[0])
                                    || node.position.y != f64::from(position[1]);
                                node.position.x = f64::from(position[0]);
                                node.position.y = f64::from(position[1]);
                                if let Some(size) = size {
                                    let resized = node.width != Some(f64::from(size[0]))
                                        || node.height != Some(f64::from(size[1]));
                                    node.width = Some(f64::from(size[0]));
                                    node.height = Some(f64::from(size[1]));
                                    studio.dirty |= resized;
                                }
                                studio.dirty |= changed;
                            }
                        }
                    }
                    for (id, body) in inline_changes {
                        studio.set_param(&id, "body".into(), bite_schema::ParamValue::String(body));
                    }
                    *selected_nodes = current_selection;
                    if ui.background_menu() {
                        *pending_wire = None;
                        let position = ui.canvas_mouse_position();
                        *creation_position = Some(Position {
                            x: f64::from(position[0]),
                            y: f64::from(position[1]),
                        });
                        open_create = true;
                    }
                    if *open_creation_requested {
                        *pending_wire = None;
                        let position = ui.canvas_mouse_position();
                        *creation_position = Some(Position {
                            x: f64::from(position[0]),
                            y: f64::from(position[1]),
                        });
                        *open_creation_requested = false;
                        open_create = true;
                    }
                    if *frames == 2 {
                        ui.navigate();
                    }
                }
            });
            if open_create {
                node_search.clear();
                ui.open_popup("Create node");
            }
            ui.popup("Create node", |ui| {
                ui.text("Create node");
                ui.next_item_full_width();
                ui.input_text("##node-search", node_search);
                let position = creation_position
                    .clone()
                    .unwrap_or(Position { x: 300.0, y: 200.0 });
                let query = node_search.to_lowercase();
                let mut created = None;
                if (query.is_empty() || "input".contains(&query))
                    && candidate_allowed(
                        studio,
                        pending_wire,
                        NodeKind::Builtin(BuiltinNodeKind::Input),
                        "",
                    )
                    && ui.button("Input")
                {
                    if pending_wire.is_some() {
                        studio.begin_transaction();
                    }
                    created = Some(studio.add_input(position.clone()));
                }
                if (query.is_empty() || "image output".contains(&query))
                    && candidate_allowed(
                        studio,
                        pending_wire,
                        NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
                        "",
                    )
                    && ui.button("Image Output")
                {
                    if pending_wire.is_some() {
                        studio.begin_transaction();
                    }
                    created = Some(studio.add_output(position.clone()));
                }
                if (query.is_empty() || "text output".contains(&query))
                    && candidate_allowed(
                        studio,
                        pending_wire,
                        NodeKind::Builtin(BuiltinNodeKind::TextOutput),
                        "",
                    )
                    && ui.button("Text Output")
                {
                    if pending_wire.is_some() {
                        studio.begin_transaction();
                    }
                    created = Some(studio.add_text_output(position.clone()));
                }
                if (query.is_empty() || "flipbook output".contains(&query))
                    && candidate_allowed(
                        studio,
                        pending_wire,
                        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput),
                        "",
                    )
                    && ui.button("Flipbook Output")
                {
                    if pending_wire.is_some() {
                        studio.begin_transaction();
                    }
                    created = Some(studio.add_flipbook_output(position.clone()));
                }
                if pending_wire.is_none()
                    && (query.is_empty() || "comment".contains(&query))
                    && ui.button("Comment")
                {
                    if pending_wire.is_some() {
                        studio.begin_transaction();
                    }
                    created = Some(studio.add_comment(position.clone()));
                }
                let mut definitions: Vec<_> = studio
                    .registry
                    .nodes
                    .values()
                    .filter(|definition| {
                        (query.is_empty()
                            || definition.definition.label.to_lowercase().contains(&query)
                            || definition.definition.id.to_lowercase().contains(&query)
                            || definition
                                .definition
                                .category
                                .to_lowercase()
                                .contains(&query))
                            && candidate_allowed(
                                studio,
                                pending_wire,
                                NodeKind::Processing(bite_schema::ProcessingNodeKind::Process),
                                &definition.definition.id,
                            )
                    })
                    .map(|definition| {
                        (
                            definition.definition.category.clone(),
                            definition.definition.id.clone(),
                            definition.definition.label.clone(),
                        )
                    })
                    .collect();
                definitions.sort();
                let mut category = String::new();
                for (next_category, id, label) in definitions {
                    if category != next_category {
                        category = next_category;
                        ui.text(&category);
                    }
                    if ui.button(&label) {
                        if pending_wire.is_some() {
                            studio.begin_transaction();
                        }
                        match studio.add_processing(&id, position.clone()) {
                            Ok(id) => created = Some(id),
                            Err(error) => studio.status = error,
                        }
                    }
                }
                if let Some(id) = created {
                    if let Some(wire) = pending_wire.take() {
                        let connection = studio
                            .workflow
                            .graph
                            .nodes
                            .iter()
                            .find(|node| node.id == wire.node)
                            .and_then(|existing| {
                                bite_core::graph::handle_type(
                                    existing,
                                    &wire.handle,
                                    wire.output,
                                    &studio.registry,
                                )
                                .ok()
                            })
                            .and_then(|wire_type| {
                                studio
                                    .workflow
                                    .graph
                                    .nodes
                                    .iter()
                                    .find(|node| node.id == id)
                                    .and_then(|node| {
                                        let (inputs, outputs) = node_ports(node, &studio.registry);
                                        let candidates = if wire.output { inputs } else { outputs };
                                        candidates.into_iter().find_map(|(handle, _)| {
                                            bite_core::graph::handle_type(
                                                node,
                                                &handle,
                                                !wire.output,
                                                &studio.registry,
                                            )
                                            .ok()
                                            .filter(|candidate| {
                                                bite_core::graph::compatible(wire_type, *candidate)
                                            })
                                            .map(|_| handle)
                                        })
                                    })
                            });
                        let result = if let Some(handle) = connection {
                            if wire.output {
                                studio.connect(&wire.node, &wire.handle, &id, &handle)
                            } else {
                                studio.connect(&id, &handle, &wire.node, &wire.handle)
                            }
                        } else {
                            Err("New node has no compatible port".into())
                        };
                        studio.finish_transaction("Created and connected node");
                        if let Err(error) = result {
                            studio.status = format!("Created node; connection rejected: {error}");
                        }
                    }
                    *selected = stable_id(&format!("node:{id}"));
                    *frames = 0;
                    ui.close_popup();
                }
            });
        });
        ui.window("Inspector", |ui| {
            if let Some(index) = studio
                .workflow
                .graph
                .nodes
                .iter()
                .position(|node| stable_id(&format!("node:{}", node.id)) == *selected)
            {
                let node = &studio.workflow.graph.nodes[index];
                ui.text(&node.data.label);
                ui.text(&format!("ID: {}", node.id));
                ui.text(&format!("Definition: {}", node.data.definition_id));
                let node_id = node.id.clone();
                let is_text_output = node.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput);
                let params = node.data.params.clone();
                let mut dialog_error = None;
                let folder_wired = studio
                    .workflow
                    .graph
                    .edges
                    .iter()
                    .any(|edge| edge.target == node_id && edge.target_handle == "folder-in");
                let has_set_input = upstream_contains_definition(
                    &studio.workflow.graph,
                    &node_id,
                    "process_as_set",
                );
                let compiled_definition = studio.registry.nodes.get(&node.data.definition_id);
                let mut definition_list = compiled_definition
                    .map(|compiled| compiled.definition.params.clone())
                    .unwrap_or_default();
                let format_parameters = if node.data.definition_id == "format_convert" {
                    params
                        .get("format")
                        .and_then(|value| match value {
                            bite_schema::ParamValue::String(format) => {
                                studio.registry.formats.get(&format.to_uppercase())
                            }
                            _ => None,
                        })
                        .map(|(format, _)| format.params.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                definition_list.extend(format_parameters.clone());
                let definitions: BTreeMap<_, _> = definition_list
                    .iter()
                    .map(|definition| (definition.name.clone(), definition.clone()))
                    .collect();
                let mut context = bite_expr::definition::default_context(&definition_list);
                for (name, value) in &params {
                    if let Some(value) = bite_expr::definition::to_value(value) {
                        context.insert(name.clone(), value);
                    }
                }
                let visible = |name: &str| {
                    let compiled_result = compiled_definition
                        .and_then(|compiled| compiled.visible.get(name))
                        .map(|expression| {
                            expression
                                .evaluate(&context)
                                .map(|value| value.truthy())
                                .unwrap_or(true)
                        });
                    let format_result = format_parameters
                        .iter()
                        .find(|parameter| parameter.name == name)
                        .and_then(|parameter| parameter.visible_when.as_deref())
                        .and_then(|source| {
                            let types = bite_expr::definition::parameter_types(&format_parameters);
                            bite_expr::Expression::compile(source, &types).ok()
                        })
                        .map(|expression| {
                            expression
                                .evaluate(&context)
                                .map(|value| value.truthy())
                                .unwrap_or(true)
                        });
                    compiled_result.or(format_result).unwrap_or(true)
                };
                let enabled = |name: &str| {
                    compiled_definition
                        .and_then(|compiled| compiled.enabled.get(name))
                        .map(|expression| {
                            expression
                                .evaluate(&context)
                                .map(|value| value.truthy())
                                .unwrap_or(true)
                        })
                        .unwrap_or(true)
                };
                let builtin_visible = |name: &str| match (node.kind.clone(), name) {
                    (NodeKind::Builtin(BuiltinNodeKind::ImageOutput), "outputPath") => {
                        !folder_wired
                    }
                    (NodeKind::Builtin(BuiltinNodeKind::ImageOutput), "customPath") => {
                        !folder_wired
                            && matches!(
                            params.get("outputPath"),
                            Some(bite_schema::ParamValue::String(value)) if value == "custom"
                            )
                    }
                    (
                        NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
                        "setOutputPrefix" | "setOutputSuffix",
                    ) => has_set_input,
                    (NodeKind::Builtin(BuiltinNodeKind::TextOutput), "customSeparator") => {
                        matches!(
                            params.get("separatorType"),
                            Some(bite_schema::ParamValue::String(value)) if value == "custom"
                        )
                    }
                    (NodeKind::Builtin(BuiltinNodeKind::TextOutput), "nextPortIndex") => false,
                    _ => true,
                };
                if folder_wired {
                    ui.text("Output folder: connected Folder Path node");
                }
                if node.kind == NodeKind::Builtin(BuiltinNodeKind::Input) {
                    ui.text(&format!("Imported images: {}", image_paths.len()));
                }
                if matches!(
                    node.kind,
                    NodeKind::Builtin(BuiltinNodeKind::Input | BuiltinNodeKind::ImageOutput)
                ) {
                    let is_input = node.kind == NodeKind::Builtin(BuiltinNodeKind::Input);
                    let fallback = if is_input {
                        &*input_path
                    } else {
                        &*output_path
                    };
                    let mut path = runtime_paths
                        .get(&node_id)
                        .cloned()
                        .unwrap_or_else(|| fallback.clone());
                    ui.text(if is_input {
                        "Run input folder"
                    } else {
                        "Run output folder"
                    });
                    ui.next_item_full_width();
                    if ui.input_text(&format!("##runtime-path-{node_id}"), &mut path) {
                        runtime_paths.insert(node_id.clone(), path.clone());
                    }
                    if ui.button(&format!("Browse run folder##{node_id}")) {
                        match dialogs::select_folder() {
                            Ok(Some(selected)) => {
                                runtime_paths.insert(
                                    node_id.clone(),
                                    selected.to_string_lossy().into_owned(),
                                );
                            }
                            Ok(None) => {}
                            Err(error) => dialog_error = Some(error),
                        }
                    }
                }
                if is_text_output {
                    ui.text("Generated text preview");
                    if image_paths.is_empty() {
                        ui.text("Import images to preview this output");
                    } else if text_preview_receiver.is_none()
                        && ui.button(&format!("Refresh preview##text-preview-{node_id}"))
                    {
                        let graph = studio.workflow.graph.clone();
                        let registry = studio.registry.clone();
                        let images: Vec<_> = image_paths.iter().take(10).cloned().collect();
                        let preview_node = node_id.clone();
                        let (send, receive) = mpsc::channel();
                        std::thread::spawn(move || {
                            let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
                            let mut host = bite_imagemagick::Magick::discover(cancelled);
                            let result = bite_core::preview::render_text(
                                &graph,
                                &registry,
                                &mut host,
                                &images,
                                &preview_node,
                            );
                            let _ = send.send((preview_node, result));
                        });
                        *text_preview_receiver = Some(receive);
                        *text_preview_result = None;
                    } else if text_preview_receiver.is_some() {
                        ui.text("Computing preview…");
                    }
                    if let Some((preview_node, result)) = text_preview_result.as_ref() {
                        if preview_node == &node_id {
                            match result {
                                Ok(lines) if lines.is_empty() => ui.text("Preview is empty"),
                                Ok(lines) => {
                                    for line in lines.iter().take(10) {
                                        ui.text(line);
                                    }
                                }
                                Err(error) => ui.text(&format!("Preview failed: {error}")),
                            }
                        }
                    }
                }
                let mut parameter_names: Vec<_> = definition_list
                    .iter()
                    .map(|definition| definition.name.clone())
                    .filter(|name| {
                        (params.contains_key(name)
                            || definitions
                                .get(name)
                                .and_then(|definition| definition.default.as_ref())
                                .is_some())
                            && visible(name)
                            && builtin_visible(name)
                            && !definitions
                                .get(name)
                                .is_some_and(|definition| definition.port_only)
                    })
                    .collect();
                parameter_names.extend(
                    params
                        .keys()
                        .filter(|name| {
                            !definitions.contains_key(*name)
                                && visible(name)
                                && builtin_visible(name)
                        })
                        .cloned(),
                );
                let mut parameter_changes = Vec::new();
                for name in parameter_names {
                    let Some(value) = params.get(&name).cloned().or_else(|| {
                        definitions
                            .get(&name)
                            .and_then(|definition| definition.default.clone())
                    }) else {
                        continue;
                    };
                    let definition = definitions.get(&name);
                    let fallback_label = human_label(&name);
                    let label = definition
                        .map(|definition| definition.label.as_str())
                        .filter(|label| !label.is_empty())
                        .unwrap_or(&fallback_label);
                    let control_id = format!("##{node_id}-{name}");
                    let editable =
                        enabled(&name) && !definition.is_some_and(|definition| definition.readonly);
                    let changed = ui.disabled(!editable, |ui| match value {
                        bite_schema::ParamValue::Number(number) => {
                            let mut edited = number as f32;
                            ui.text(label);
                            ui.next_item_full_width();
                            let changed = if definition.is_some_and(|definition| {
                                definition.widget == Some(WidgetType::Slider)
                            }) {
                                ui.slider_float(
                                    &control_id,
                                    &mut edited,
                                    definition
                                        .and_then(|definition| definition.min)
                                        .unwrap_or(0.0) as f32,
                                    definition
                                        .and_then(|definition| definition.max)
                                        .unwrap_or(100.0)
                                        as f32,
                                )
                            } else {
                                ui.drag_float(&control_id, &mut edited)
                            };
                            changed.then_some(bite_schema::ParamValue::Number(f64::from(edited)))
                        }
                        bite_schema::ParamValue::Int(number) => {
                            let mut edited = number as f32;
                            ui.text(label);
                            ui.next_item_full_width();
                            let range = definition
                                .filter(|definition| definition.widget == Some(WidgetType::Slider))
                                .map(|definition| {
                                    (
                                        definition.min.unwrap_or(0.0) as f32,
                                        definition.max.unwrap_or(100.0) as f32,
                                    )
                                })
                                .or(match name.as_str() {
                                    "thumbnailSize" => Some((64.0, 1024.0)),
                                    "cols" | "rows" => Some((1.0, 64.0)),
                                    "cellWidth" | "cellHeight" => Some((1.0, 8192.0)),
                                    _ => None,
                                });
                            let changed = if let Some((minimum, maximum)) = range {
                                ui.slider_float(&control_id, &mut edited, minimum, maximum)
                            } else {
                                ui.drag_float(&control_id, &mut edited)
                            };
                            changed.then_some(bite_schema::ParamValue::Int(edited.round() as i64))
                        }
                        bite_schema::ParamValue::String(mut text) => {
                            let options = definition
                                .filter(|definition| {
                                    definition.kind == ParamType::Enum
                                        || definition.widget == Some(WidgetType::Dropdown)
                                })
                                .map(|definition| definition.options.clone())
                                .unwrap_or_else(|| builtin_options(&name));
                            ui.text(label);
                            ui.next_item_full_width();
                            let mut changed = if options.is_empty() {
                                ui.input_text(&control_id, &mut text)
                                    .then_some(bite_schema::ParamValue::String(text))
                            } else {
                                let mut selected = options
                                    .iter()
                                    .position(|option| option == &text)
                                    .unwrap_or(0);
                                let displayed = definition
                                    .filter(|definition| {
                                        definition.labels.len() == definition.options.len()
                                    })
                                    .map(|definition| definition.labels.clone())
                                    .unwrap_or_else(|| {
                                        let labels = builtin_labels(&name);
                                        if labels.len() == options.len() {
                                            labels
                                        } else {
                                            options.clone()
                                        }
                                    });
                                ui.combo(&control_id, &mut selected, &displayed).then(|| {
                                    bite_schema::ParamValue::String(options[selected].clone())
                                })
                            };
                            if name == "cliName" {
                                if let Some(bite_schema::ParamValue::String(value)) = &mut changed {
                                    *value = sanitize_cli_name(value);
                                }
                                let current_name = params
                                    .get("cliName")
                                    .and_then(|value| match value {
                                        bite_schema::ParamValue::String(value) => {
                                            Some(value.as_str())
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or("");
                                let conflict = !current_name.is_empty()
                                    && studio.workflow.graph.nodes.iter().any(|other| {
                                        other.id != node_id
                                            && other.data.params.get("cliName").is_some_and(
                                                |value| {
                                                    value
                                                        == &bite_schema::ParamValue::String(
                                                            current_name.into(),
                                                        )
                                                },
                                            )
                                    });
                                if conflict {
                                    ui.text("CLI name is already used by another workflow node");
                                } else if !current_name.is_empty() {
                                    ui.text(&format!("Export flag: --{current_name}"));
                                }
                            }
                            let browse_folder = name == "folderPath"
                                || (node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput)
                                    && name == "customPath");
                            if browse_folder && ui.button("Browse folder") {
                                match dialogs::select_folder() {
                                    Ok(Some(path)) => {
                                        changed = Some(bite_schema::ParamValue::String(
                                            path.to_string_lossy().into_owned(),
                                        ));
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        dialog_error = Some(error);
                                    }
                                }
                            }
                            let browse_output = match (node.kind.clone(), name.as_str()) {
                                (NodeKind::Builtin(BuiltinNodeKind::TextOutput), "outputPath") => {
                                    Some(("txt", "Text output"))
                                }
                                (
                                    NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput),
                                    "flipbookOutputPath",
                                ) => Some(("png", "Flipbook image")),
                                _ => None,
                            };
                            if let Some((extension, description)) = browse_output {
                                if ui.button("Browse output file") {
                                    let default = params
                                        .get(&name)
                                        .and_then(|value| match value {
                                            bite_schema::ParamValue::String(value)
                                                if !value.is_empty() =>
                                            {
                                                Some(PathBuf::from(value))
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or_else(|| {
                                            PathBuf::from(format!("output.{extension}"))
                                        });
                                    match dialogs::save_file(&default, extension, description) {
                                        Ok(Some(path)) => {
                                            changed = Some(bite_schema::ParamValue::String(
                                                path.to_string_lossy().into_owned(),
                                            ));
                                        }
                                        Ok(None) => {}
                                        Err(error) => dialog_error = Some(error),
                                    }
                                }
                            }
                            changed
                        }
                        bite_schema::ParamValue::Bool(mut value) => ui
                            .checkbox(label, &mut value)
                            .then_some(bite_schema::ParamValue::Bool(value)),
                        bite_schema::ParamValue::Vector(values) => {
                            ui.text(label);
                            ui.next_item_full_width();
                            if name == "bgColor"
                                || definition.is_some_and(|definition| {
                                    definition.kind == ParamType::Color
                                        || definition.widget == Some(WidgetType::ColorPicker)
                                })
                            {
                                let mut color = [
                                    values.first().copied().unwrap_or(0.0) as f32,
                                    values.get(1).copied().unwrap_or(0.0) as f32,
                                    values.get(2).copied().unwrap_or(0.0) as f32,
                                    values.get(3).copied().unwrap_or(1.0) as f32,
                                ];
                                ui.color_edit4(&control_id, &mut color).then_some(
                                    bite_schema::ParamValue::Vector(
                                        color.into_iter().map(f64::from).collect(),
                                    ),
                                )
                            } else {
                                let mut edited: Vec<_> =
                                    values.iter().copied().map(|value| value as f32).collect();
                                ui.drag_float_n(&control_id, &mut edited).then_some(
                                    bite_schema::ParamValue::Vector(
                                        edited.into_iter().map(f64::from).collect(),
                                    ),
                                )
                            }
                        }
                        bite_schema::ParamValue::Structured(
                            bite_schema::StructuredParam::SetSuffixes { mut suffixes },
                        ) => {
                            ui.text(label);
                            let mut changed = false;
                            let mut remove = None;
                            for (index, suffix) in suffixes.iter_mut().enumerate() {
                                ui.next_item_full_width();
                                changed |=
                                    ui.input_text(&format!("##{node_id}-{name}-{index}"), suffix);
                                if ui.button(&format!("Remove suffix##{node_id}-{index}")) {
                                    remove = Some(index);
                                }
                            }
                            if let Some(index) = remove {
                                suffixes.remove(index);
                                changed = true;
                            }
                            if ui.button(&format!("Add suffix##{node_id}")) {
                                suffixes.push(String::new());
                                changed = true;
                            }
                            let prefix = params
                                .get("prefix")
                                .and_then(|value| match value {
                                    bite_schema::ParamValue::String(value) => Some(value.as_str()),
                                    _ => None,
                                })
                                .unwrap_or("");
                            let active: Vec<_> = suffixes
                                .iter()
                                .filter(|suffix| !suffix.is_empty())
                                .collect();
                            if !active.is_empty() {
                                let mut groups: BTreeMap<String, BTreeSet<String>> =
                                    BTreeMap::new();
                                for path in image_paths.iter() {
                                    let stem =
                                        path.file_stem().unwrap_or_default().to_string_lossy();
                                    let Some(rest) = stem.strip_prefix(prefix) else {
                                        continue;
                                    };
                                    for suffix in &active {
                                        if let Some(middle) = rest.strip_suffix(suffix.as_str()) {
                                            groups
                                                .entry(middle.into())
                                                .or_default()
                                                .insert((*suffix).clone());
                                            break;
                                        }
                                    }
                                }
                                let complete = groups
                                    .values()
                                    .filter(|found| found.len() == active.len())
                                    .count();
                                ui.text(&format!(
                                    "Matched sets: {complete}/{} complete",
                                    groups.len()
                                ));
                                for (middle, found) in groups.iter().take(6) {
                                    ui.text(&format!(
                                        "{}{middle}: {}/{}",
                                        prefix,
                                        found.len(),
                                        active.len()
                                    ));
                                }
                            }
                            changed.then_some(bite_schema::ParamValue::Structured(
                                bite_schema::StructuredParam::SetSuffixes { suffixes },
                            ))
                        }
                        bite_schema::ParamValue::Structured(
                            bite_schema::StructuredParam::RenameBlocks { mut blocks },
                        ) => {
                            ui.text(label);
                            let mut changed = false;
                            let mut remove = None;
                            let mut move_block = None;
                            let block_count = blocks.len();
                            for (index, block) in blocks.iter_mut().enumerate() {
                                match block {
                                    bite_schema::RenameBlock::Text { value } => {
                                        ui.text(&format!("Text block {}", index + 1));
                                        ui.next_item_full_width();
                                        changed |= ui.input_text(
                                            &format!("##{node_id}-{name}-text-{index}"),
                                            value,
                                        );
                                    }
                                    bite_schema::RenameBlock::Number { start, pad } => {
                                        ui.text(&format!("Number block {}", index + 1));
                                        let mut start_value = *start as f32;
                                        let mut pad_value = *pad as f32;
                                        changed |= ui.drag_float(
                                            &format!("Start##{node_id}-{index}"),
                                            &mut start_value,
                                        );
                                        changed |= ui.drag_float(
                                            &format!("Pad##{node_id}-{index}"),
                                            &mut pad_value,
                                        );
                                        *start = f64::from(start_value.max(0.0).round());
                                        *pad = f64::from(pad_value.clamp(1.0, 8.0).round());
                                    }
                                    bite_schema::RenameBlock::Oldname { find, replace_with } => {
                                        ui.text(&format!("Old name block {}", index + 1));
                                        ui.next_item_full_width();
                                        changed |= ui
                                            .input_text(&format!("Find##{node_id}-{index}"), find);
                                        ui.next_item_full_width();
                                        changed |= ui.input_text(
                                            &format!("Replace##{node_id}-{index}"),
                                            replace_with,
                                        );
                                    }
                                }
                                if index > 0 && ui.button(&format!("Up##{node_id}-{index}")) {
                                    move_block = Some((index, index - 1));
                                }
                                if index > 0 {
                                    ui.same_line();
                                }
                                if index + 1 < block_count
                                    && ui.button(&format!("Down##{node_id}-{index}"))
                                {
                                    move_block = Some((index, index + 1));
                                }
                                if index + 1 < block_count {
                                    ui.same_line();
                                }
                                if ui.button(&format!("Remove##{node_id}-{index}")) {
                                    remove = Some(index);
                                }
                            }
                            if let Some((from, to)) = move_block {
                                blocks.swap(from, to);
                                changed = true;
                            }
                            if let Some(index) = remove {
                                blocks.remove(index);
                                changed = true;
                            }
                            if ui.button(&format!("Add Text##{node_id}")) {
                                blocks.push(bite_schema::RenameBlock::Text {
                                    value: String::new(),
                                });
                                changed = true;
                            }
                            ui.same_line();
                            if ui.button(&format!("Add Number##{node_id}")) {
                                blocks.push(bite_schema::RenameBlock::Number {
                                    start: 1.0,
                                    pad: 2.0,
                                });
                                changed = true;
                            }
                            ui.same_line();
                            if ui.button(&format!("Add Old Name##{node_id}")) {
                                blocks.push(bite_schema::RenameBlock::Oldname {
                                    find: String::new(),
                                    replace_with: String::new(),
                                });
                                changed = true;
                            }
                            ui.text("Preview");
                            let examples = ["photo_001.jpg", "IMG_5432.jpg", "vacation shot.png"];
                            if image_paths.is_empty() {
                                for (index, original) in examples.iter().enumerate() {
                                    ui.text(&format!(
                                        "{original} -> {}",
                                        bite_core::execution::rename(original, &blocks, index)
                                    ));
                                }
                            } else {
                                for (index, path) in image_paths.iter().take(10).enumerate() {
                                    let original =
                                        path.file_name().unwrap_or_default().to_string_lossy();
                                    ui.text(&format!(
                                        "{original} -> {}",
                                        bite_core::execution::rename(&original, &blocks, index)
                                    ));
                                }
                            }
                            changed.then_some(bite_schema::ParamValue::Structured(
                                bite_schema::StructuredParam::RenameBlocks { blocks },
                            ))
                        }
                        bite_schema::ParamValue::Structured(
                            bite_schema::StructuredParam::TextSlots { mut slots },
                        ) => {
                            ui.text(&format!("{label}: {} text ports", slots.len()));
                            let mut changed = false;
                            let connected: BTreeSet<_> = studio
                                .workflow
                                .graph
                                .edges
                                .iter()
                                .filter(|edge| edge.target == node_id)
                                .filter_map(|edge| {
                                    edge.target_handle.strip_prefix("txo:").map(str::to_owned)
                                })
                                .collect();
                            let mut move_slot = None;
                            let mut remove_slot = None;
                            for (index, slot) in slots.iter().enumerate() {
                                let source_label = studio
                                    .workflow
                                    .graph
                                    .edges
                                    .iter()
                                    .find(|edge| {
                                        edge.target == node_id
                                            && edge.target_handle == format!("txo:{slot}")
                                    })
                                    .and_then(|edge| {
                                        studio
                                            .workflow
                                            .graph
                                            .nodes
                                            .iter()
                                            .find(|source| source.id == edge.source)
                                    })
                                    .map(|source| source.data.label.as_str());
                                ui.text(&format!(
                                    "Text {} — {}",
                                    index + 1,
                                    source_label.unwrap_or("empty connection")
                                ));
                                if index > 0 && ui.button(&format!("Up##text-{slot}")) {
                                    move_slot = Some((index, index - 1));
                                }
                                if index > 0 {
                                    ui.same_line();
                                }
                                if index + 1 < slots.len()
                                    && ui.button(&format!("Down##text-{slot}"))
                                {
                                    move_slot = Some((index, index + 1));
                                }
                                if index + 1 < slots.len() {
                                    ui.same_line();
                                }
                                if !connected.contains(slot)
                                    && slots.len() > 1
                                    && ui.button(&format!("Remove##text-{slot}"))
                                {
                                    remove_slot = Some(index);
                                }
                            }
                            if let Some((from, to)) = move_slot {
                                slots.swap(from, to);
                                changed = true;
                            }
                            if let Some(index) = remove_slot {
                                slots.remove(index);
                                changed = true;
                            }
                            changed.then_some(bite_schema::ParamValue::Structured(
                                bite_schema::StructuredParam::TextSlots { slots },
                            ))
                        }
                        other => {
                            ui.text(&format!("{name}: {other:?}"));
                            None
                        }
                    });
                    if let Some(value) = changed {
                        parameter_changes.push((name, value));
                    }
                }
                if node.kind == NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) {
                    let integer = |name: &str, fallback: i64| {
                        params
                            .get(name)
                            .and_then(|value| match value {
                                bite_schema::ParamValue::Int(value) => Some(*value),
                                _ => None,
                            })
                            .unwrap_or(fallback)
                            .max(1)
                    };
                    let cols = integer("cols", 4);
                    let rows = integer("rows", 4);
                    let cell_width = integer("cellWidth", 128);
                    let cell_height = integer("cellHeight", 128);
                    let capacity = cols.saturating_mul(rows);
                    let images = i64::try_from(image_paths.len()).unwrap_or(i64::MAX);
                    ui.text(&format!(
                        "Atlas: {} × {} px; {capacity} cells ({cols} × {rows})",
                        cols.saturating_mul(cell_width),
                        rows.saturating_mul(cell_height)
                    ));
                    if images > capacity {
                        ui.text(&format!(
                            "Images: {images}; {} will be truncated",
                            images - capacity
                        ));
                    } else {
                        ui.text(&format!(
                            "Images: {images}; {} cells unfilled",
                            capacity - images
                        ));
                    }
                }
                for (name, value) in parameter_changes {
                    if studio.set_param(&node_id, name, value)
                        && image_paths.get(*selected_image).is_some()
                    {
                        if is_text_output {
                            *text_preview_result = None;
                        }
                        *preview_requested = Some(*selected_image);
                        studio.status = "Parameter changed; refreshing preview…".into();
                    }
                }
                if let Some(error) = dialog_error {
                    studio.status = format!("Folder dialog failed: {error}");
                }
                if let Some(values) = resolved_values.get(&node_id) {
                    for (name, value) in values {
                        ui.text(&format!("{name} = {}", value.text()));
                    }
                }
            } else {
                ui.text("Select a workflow node");
            }
            if *technical_demo {
                ui.drag_float("Value", value);
                ui.input_text("Text", text);
            }
            ui.text(dropped);
        });
        ui.window("Preview", |ui| {
            ui.image(
                preview_texture
                    .or_else(|| thumbnails.first().copied())
                    .unwrap_or(2),
                256.,
                256.,
            );
        });
        ui.window("Filmstrip", |ui| {
            if let Some(input) = active_input.as_ref().and_then(|id| {
                studio
                    .workflow
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *id)
            }) {
                ui.text(&format!("Input: {}", input.data.label));
            }
            let displayed = thumbnails.clone();
            if let Some(path) = image_paths.get(*selected_image) {
                ui.text(&format!("Selected: {}", path.display()));
            }
            for (index, texture) in displayed.into_iter().enumerate() {
                if ui.image_button(&format!("filmstrip-{index}"), texture, 64., 64.)
                    && index < thumbnails.len()
                {
                    *preview_requested = Some(index);
                }
                ui.same_line();
            }
        });
        *frames += 1;
        frame.render()
    }
}
struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    demo: Demo,
    last: Instant,
    settle: u8,
}
#[derive(Default)]
struct App {
    state: Option<State>,
    error: Option<String>,
}
impl App {
    fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("BITE")
                        .with_inner_size(winit::dpi::LogicalSize::new(1600., 1000.)),
                )
                .map_err(|e| e.to_string())?,
        );
        window.set_ime_allowed(true);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No surface configuration")?;
        config.format = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        let mut renderer = pollster::block_on(Renderer::new(&adapter, config.format))?;
        surface.configure(&renderer.device, &config);
        let demo = Demo::new(&mut renderer, window.scale_factor() as f32, false)?;
        window.request_redraw();
        self.state = Some(State {
            window,
            surface,
            config,
            renderer,
            demo,
            last: Instant::now(),
            settle: 8,
        });
        Ok(())
    }
}
fn key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Space => Key::Space,
        KeyCode::Enter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::KeyF => Key::F,
        _ => return None,
    })
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            if let Err(e) = self.init(event_loop) {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else {
            return;
        };
        let scale = s.window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => {
                if s.demo.studio.dirty {
                    s.demo.pending_action = Some(PendingAction::Exit);
                    s.window.request_redraw();
                } else {
                    event_loop.exit();
                }
                return;
            }
            WindowEvent::RedrawRequested => {
                if s.config.width == 0 || s.config.height == 0 {
                    return;
                }
                let frame = match s.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded => return,
                    wgpu::CurrentSurfaceTexture::Outdated => {
                        s.surface.configure(&s.renderer.device, &s.config);
                        s.window.request_redraw();
                        return;
                    }
                    e => {
                        self.error = Some(format!("Surface unavailable: {e:?}"));
                        event_loop.exit();
                        return;
                    }
                };
                let now = Instant::now();
                let data = s.demo.draw(
                    s.config.width,
                    s.config.height,
                    scale,
                    (now - s.last).as_secs_f32(),
                );
                s.last = now;
                if s.demo.import_requested {
                    s.demo.import_requested = false;
                    if let Err(error) = s.demo.import_images(&mut s.renderer) {
                        s.demo.studio.status = format!("Import failed: {error}");
                    }
                    s.window.request_redraw();
                }
                if let Some(index) = s.demo.preview_requested.take() {
                    if let Err(error) = s.demo.refresh_preview(&mut s.renderer, index) {
                        s.demo.studio.status = format!("Preview failed: {error}");
                    }
                    s.window.request_redraw();
                }
                s.renderer.render(
                    &frame.texture.create_view(&Default::default()),
                    &data,
                    s.config.width,
                    s.config.height,
                    scale,
                );
                s.window.pre_present_notify();
                s.renderer.queue.present(frame);
                if s.demo.exit_requested {
                    event_loop.exit();
                    return;
                }
                if s.settle > 0 {
                    s.settle -= 1;
                    s.window.request_redraw();
                }
                return;
            }
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                s.config.width = size.width;
                s.config.height = size.height;
                s.surface.configure(&s.renderer.device, &s.config);
            }
            WindowEvent::CursorMoved { position, .. } => s
                .demo
                .context
                .mouse_position(position.x as f32 / scale, position.y as f32 / scale),
            WindowEvent::CursorLeft { .. } => s.demo.context.mouse_position(-f32::MAX, -f32::MAX),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    _ => 4,
                };
                s.demo
                    .context
                    .mouse_button(button, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => {
                        (p.x as f32 / scale / 40., p.y as f32 / scale / 40.)
                    }
                };
                s.demo.context.mouse_wheel(x, y);
            }
            WindowEvent::ModifiersChanged(m) => {
                let m = m.state();
                s.demo.control_down = m.control_key() || m.super_key();
                s.demo.shift_down = m.shift_key();
                for (k, v) in [
                    (Key::Ctrl, m.control_key()),
                    (Key::Shift, m.shift_key()),
                    (Key::Alt, m.alt_key()),
                    (Key::Super, m.super_key()),
                ] {
                    s.demo.context.key(k, v);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(k) = key(code) {
                        s.demo.context.key(k, event.state == ElementState::Pressed);
                    }
                    if event.state == ElementState::Pressed && !s.demo.context.want_text_input() {
                        let selection = s.demo.selected_nodes.clone();
                        let mut select_pasted = |pasted: Vec<String>| {
                            if let Some(id) = pasted.first() {
                                s.demo.selected = stable_id(&format!("node:{id}"));
                                s.demo.frames = 0;
                            }
                        };
                        match code {
                            KeyCode::Delete | KeyCode::Backspace => {
                                if s.demo.studio.delete_selection(&selection) {
                                    let deleting_active = s
                                        .demo
                                        .active_input
                                        .as_ref()
                                        .is_some_and(|id| selection.contains(id));
                                    for id in &selection {
                                        s.demo.runtime_paths.remove(id);
                                        s.demo.input_media.remove(id);
                                    }
                                    if deleting_active {
                                        s.demo.active_input = None;
                                        s.demo.image_paths.clear();
                                        s.demo.thumbnails.clear();
                                        s.demo.selected_image = 0;
                                        s.demo.preview_texture = None;
                                        s.demo.resolved_values.clear();
                                        s.demo.text_preview_receiver = None;
                                        s.demo.text_preview_result = None;
                                    }
                                    s.demo.selected = 0;
                                    s.demo.selected_nodes.clear();
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyZ if s.demo.control_down && s.demo.shift_down => {
                                if s.demo.studio.redo() {
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyZ if s.demo.control_down => {
                                if s.demo.studio.undo() {
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyY if s.demo.control_down => {
                                if s.demo.studio.redo() {
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyC if s.demo.control_down => {
                                copy_selection(&mut s.demo.studio, &selection);
                            }
                            KeyCode::KeyV if s.demo.control_down => {
                                let pasted = paste_selection(&mut s.demo.studio);
                                select_pasted(pasted);
                            }
                            KeyCode::KeyD if s.demo.control_down => {
                                let pasted = s.demo.studio.duplicate_selection(&selection);
                                select_pasted(pasted);
                            }
                            KeyCode::KeyG if s.demo.control_down && s.demo.shift_down => {
                                if s.demo.studio.ungroup_selection(&selection) {
                                    s.demo.selected = 0;
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyG if s.demo.control_down => {
                                if let Some(id) = s.demo.studio.group_selection(&selection) {
                                    s.demo.selected = stable_id(&format!("node:{id}"));
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyS if s.demo.control_down && s.demo.shift_down => {
                                let default = PathBuf::from(&s.demo.workflow_path);
                                match dialogs::save_file(&default, "bite", "BITE workflow") {
                                    Ok(Some(path)) => match s.demo.studio.save(&path) {
                                        Ok(()) => {
                                            s.demo.workflow_path =
                                                path.to_string_lossy().into_owned()
                                        }
                                        Err(error) => {
                                            s.demo.studio.status = format!("Save failed: {error}")
                                        }
                                    },
                                    Ok(None) => {}
                                    Err(error) => {
                                        s.demo.studio.status =
                                            format!("Save dialog failed: {error}")
                                    }
                                }
                            }
                            KeyCode::KeyS if s.demo.control_down => {
                                let path = PathBuf::from(&s.demo.workflow_path);
                                if let Err(error) = s.demo.studio.save(&path) {
                                    s.demo.studio.status = format!("Save failed: {error}");
                                }
                            }
                            KeyCode::KeyN if s.demo.control_down => {
                                if s.demo.studio.dirty {
                                    s.demo.pending_action = Some(PendingAction::New);
                                } else {
                                    let registry = s.demo.studio.registry.clone();
                                    s.demo.studio = Studio::blank(registry);
                                    s.demo.studio.add_input(Position { x: 0.0, y: 100.0 });
                                    s.demo.studio.add_output(Position { x: 700.0, y: 100.0 });
                                    s.demo.studio.mark_clean();
                                    s.demo.workflow_path = "workflow.bite".into();
                                    s.demo.runtime_paths.clear();
                                    s.demo.clear_media();
                                    s.demo.selected = 0;
                                    s.demo.selected_nodes.clear();
                                    s.demo.frames = 0;
                                }
                            }
                            KeyCode::KeyO if s.demo.control_down => {
                                match dialogs::open_workflow() {
                                    Ok(Some(path)) => {
                                        s.demo.workflow_path = path.to_string_lossy().into_owned();
                                        if s.demo.studio.dirty {
                                            s.demo.pending_action = Some(PendingAction::Open);
                                        } else {
                                            match Studio::open(
                                                s.demo.studio.registry.clone(),
                                                &path,
                                            ) {
                                                Ok(studio) => {
                                                    s.demo.studio = studio;
                                                    s.demo.runtime_paths.clear();
                                                    s.demo.clear_media();
                                                    s.demo.selected = 0;
                                                    s.demo.selected_nodes.clear();
                                                    s.demo.frames = 0;
                                                }
                                                Err(error) => {
                                                    s.demo.studio.status =
                                                        format!("Open failed: {error}")
                                                }
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        s.demo.studio.status =
                                            format!("Open dialog failed: {error}")
                                    }
                                }
                            }
                            KeyCode::Space | KeyCode::Tab if !s.demo.control_down => {
                                s.demo.open_creation_requested = true;
                            }
                            _ => {}
                        }
                    }
                }
                if event.state == ElementState::Pressed {
                    if let Some(text) = event.text {
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        s.demo.context.text_input(&text);
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => s.demo.context.text_input(&text),
            WindowEvent::Focused(focused) => {
                s.demo.context.focus(focused);
                if focused {
                    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
                    if let Err(error) = s.demo.studio.reload_registry(&root) {
                        s.demo.studio.status = format!("Definition reload failed: {error}");
                    }
                }
            }
            WindowEvent::DroppedFile(path) => {
                s.demo.dropped = path.display().to_string();
                if path.extension().and_then(|extension| extension.to_str()) == Some("bite") {
                    match Studio::open(s.demo.studio.registry.clone(), &path) {
                        Ok(studio) => {
                            s.demo.workflow_path = path.to_string_lossy().into_owned();
                            s.demo.studio = studio;
                            s.demo.runtime_paths.clear();
                            s.demo.clear_media();
                            s.demo.frames = 0;
                        }
                        Err(error) => s.demo.studio.status = format!("Open failed: {error}"),
                    }
                } else if path.is_dir() {
                    s.demo.input_path = path.to_string_lossy().into_owned();
                    s.demo.studio.status = format!("Input folder: {}", path.display());
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                match s.demo.context.set_scale(scale_factor as f32) {
                    Ok((width, height, pixels)) => {
                        s.renderer.texture(1, width, height, &pixels);
                        s.demo.context.set_font_texture(1);
                        s.demo.studio.status = format!("UI scale: {scale_factor:.2}×");
                    }
                    Err(error) => s.demo.studio.status = format!("DPI update failed: {error}"),
                }
            }
            _ => return,
        }
        s.settle = 2;
        s.window.request_redraw();
    }
}
fn smoke(path: &str, workflow: Option<&str>, import_folder: Option<&str>) -> Result<(), String> {
    let scale = std::env::var("BITE_SMOKE_SCALE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value >= 1.0 && *value <= 4.0)
        .unwrap_or(1.0);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    println!("Adapter: {:?}", adapter.get_info());
    let mut renderer =
        pollster::block_on(Renderer::new(&adapter, wgpu::TextureFormat::Rgba8Unorm))?;
    let mut demo = Demo::new(&mut renderer, scale, workflow.is_none())?;
    if let Some(workflow) = workflow {
        demo.studio = Studio::open(
            demo.studio.registry.clone(),
            PathBuf::from(workflow).as_path(),
        )?;
        demo.workflow_path = workflow.into();
        if let Some(node) = demo
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| node.data.definition_id == "resize")
        {
            demo.selected = stable_id(&format!("node:{}", node.id));
        }
        demo.frames = 0;
    }
    if let Some(import_folder) = import_folder {
        demo.input_path = import_folder.into();
        demo.import_images(&mut renderer)?;
        if demo.image_paths.len() > 1 {
            demo.refresh_preview(&mut renderer, 1)?;
        }
    }
    let (width, height) = (1600, 1000);
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("prototype smoke"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    for _ in 0..8 {
        let data = demo.draw(width, height, scale, 1. / 60.);
        renderer.render(&view, &data, width, height, scale);
    }
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * height) as u64,
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
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let data = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let pixels: Vec<u8> = data
        .chunks(stride as usize)
        .flat_map(|row| row[..(width * 4) as usize].iter().copied())
        .collect();
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut png = png::Encoder::new(file, width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    println!("Rendered native GUI smoke test to {path}");
    Ok(())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--functional-run-smoke") {
        let workflow = PathBuf::from(
            args.get(2)
                .ok_or("--functional-run-smoke needs a workflow path")?,
        );
        let input = PathBuf::from(
            args.get(3)
                .ok_or("--functional-run-smoke needs an input folder")?,
        );
        let output = PathBuf::from(
            args.get(4)
                .ok_or("--functional-run-smoke needs an output folder")?,
        );
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut studio = Studio::open(Studio::load_registry(&root)?, &workflow)?;
        let options = studio.run_options(&input, &output);
        let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
        let result = bite_core::execution::run_workflow(
            &studio.workflow.graph,
            &studio.registry,
            &mut host,
            &options,
            &mut |_, _, _| {},
        )?;
        studio.status = format!("Run complete: {} processed", result.processed);
        println!("{}", studio.status);
        return Ok(());
    }
    if args.get(1).is_some_and(|s| s == "--smoke") {
        return smoke(
            args.get(2).ok_or("--smoke needs a PNG output path")?,
            None,
            None,
        );
    }
    if args.get(1).is_some_and(|s| s == "--functional-smoke") {
        return smoke(
            args.get(2)
                .ok_or("--functional-smoke needs a PNG output path")?,
            Some(
                args.get(3)
                    .ok_or("--functional-smoke needs a workflow path")?,
            ),
            args.get(4).map(String::as_str),
        );
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(e) = app.error {
        Err(e)
    } else {
        Ok(())
    }
}
