//! Phase 8 technical smoke mode plus the Phase 9 functional native GUI.
mod renderer;
mod studio;
use bite_core::execution::ImageHost;
use bite_imgui::{Context, Key};
use bite_schema::{BuiltinNodeKind, NodeKind, Position};
use renderer::Renderer;
use std::{
    collections::BTreeMap,
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

struct Demo {
    context: Context,
    text: String,
    value: f32,
    dropped: String,
    links: Vec<(u64, u64, u64)>,
    frames: u64,
    selected: u64,
    studio: Studio,
    workflow_path: String,
    input_path: String,
    output_path: String,
    runner: Option<RunState>,
    technical_demo: bool,
    import_requested: bool,
    preview_requested: Option<usize>,
    image_paths: Vec<PathBuf>,
    thumbnails: Vec<u64>,
    preview_texture: Option<u64>,
    resolved_values: BTreeMap<String, bite_expr::Context>,
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
        NodeKind::Builtin(
            BuiltinNodeKind::ImageOutput
            | BuiltinNodeKind::TextOutput
            | BuiltinNodeKind::FlipbookOutput,
        ) => (vec![("in:input".into(), "Image".into())], Vec::new()),
        _ => registry
            .nodes
            .get(&node.data.definition_id)
            .map(|definition| {
                (
                    definition
                        .definition
                        .inputs
                        .iter()
                        .map(|port| (format!("in:{}", port.name), port.label.clone()))
                        .collect(),
                    definition
                        .definition
                        .outputs
                        .iter()
                        .map(|port| (format!("out:{}", port.name), port.label.clone()))
                        .collect(),
                )
            })
            .unwrap_or_default(),
    }
}
impl Demo {
    fn new(renderer: &mut Renderer, scale: f32, technical_demo: bool) -> Result<Self, String> {
        let mut context = Context::with_scale(scale)?;
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
            studio.dirty = false;
        }
        let workflow_path = initial.unwrap_or_else(|| "workflow.bite".into());
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
            studio,
            workflow_path,
            input_path: root.join("test_images").to_string_lossy().into_owned(),
            output_path: root
                .join("test-workflows/out/native-gui")
                .to_string_lossy()
                .into_owned(),
            runner: None,
            technical_demo,
            import_requested: false,
            preview_requested: None,
            image_paths: Vec::new(),
            thumbnails: Vec::new(),
            preview_texture: None,
            resolved_values: BTreeMap::new(),
        })
    }

    fn import_images(&mut self, renderer: &mut Renderer) -> Result<(), String> {
        let root = PathBuf::from(&self.input_path);
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let paths = bite_imagemagick::import::scan_folder(&root, false, 8, &cancelled)?;
        let cache_dir = std::env::temp_dir().join("bite-native-thumbnails");
        let mut cache = bite_imagemagick::import::ThumbnailCache::new(cache_dir.clone());
        cache.jobs = 8;
        let mut host = bite_imagemagick::Magick::discover(Arc::new(cancelled));
        let images = cache.load_batch(&mut host, &paths, 128)?;
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
            let texture = 100 + index as u64;
            renderer.texture(texture, width, height, &pixels);
            self.thumbnails.push(texture);
        }
        if !self.image_paths.is_empty() {
            self.refresh_preview(renderer, 0)?;
        }
        self.studio.status = format!("Imported {} images", images.len());
        Ok(())
    }

    fn refresh_preview(&mut self, renderer: &mut Renderer, index: usize) -> Result<(), String> {
        let image = self
            .image_paths
            .get(index)
            .ok_or_else(|| format!("Image {index} is no longer in the filmstrip"))?;
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut host = bite_imagemagick::Magick::discover(cancelled);
        let preview = bite_core::preview::render(
            &self.studio.workflow.graph,
            &self.studio.registry,
            &mut host,
            image,
            None,
        )?;
        let (width, height, pixels) = decode_png(&preview.png)?;
        renderer.texture(90, width, height, &pixels);
        self.preview_texture = Some(90);
        self.resolved_values = preview.resolved_values;
        self.studio.status = format!("Previewing {}", image.display());
        Ok(())
    }
    fn draw(&mut self, width: u32, height: u32, scale: f32, delta: f32) -> bite_imgui::DrawData {
        let Self {
            context,
            text,
            value,
            dropped,
            links,
            frames,
            selected,
            studio,
            workflow_path,
            input_path,
            output_path,
            runner,
            technical_demo,
            import_requested,
            preview_requested,
            image_paths: _,
            thumbnails,
            preview_texture,
            resolved_values,
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
        let mut frame = context.frame(
            width as f32 / scale,
            height as f32 / scale,
            scale,
            delta.clamp(0.001, 0.1),
        );
        let mut ui = frame.ui();
        ui.dockspace();
        ui.window("Workflow", |ui| {
            ui.input_text("Workflow path", workflow_path);
            if ui.button("Open") {
                match Studio::open(
                    studio.registry.clone(),
                    PathBuf::from(&*workflow_path).as_path(),
                ) {
                    Ok(opened) => {
                        *studio = opened;
                        *frames = 0;
                    }
                    Err(error) => studio.status = format!("Open failed: {error}"),
                }
            }
            ui.same_line();
            if ui.button("Save") {
                if let Err(error) = studio.save(PathBuf::from(&*workflow_path).as_path()) {
                    studio.status = format!("Save failed: {error}");
                }
            }
            ui.input_text("Input folder", input_path);
            ui.input_text("Output folder", output_path);
            if ui.button("Import images") {
                *import_requested = true;
                studio.status = "Importing images…".into();
            }
            if runner.is_none() && ui.button("Run") {
                let graph = studio.workflow.graph.clone();
                let registry = studio.registry.clone();
                let options = studio.run_options(
                    PathBuf::from(&*input_path).as_path(),
                    PathBuf::from(&*output_path).as_path(),
                );
                let cancelled = options.cancelled.clone();
                let (send, receive) = mpsc::channel();
                std::thread::spawn(move || {
                    let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
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
                if ui.button("Input") {
                    studio.add_input(Position { x: 0.0, y: 0.0 });
                }
                if ui.button("Image Output") {
                    studio.add_output(Position { x: 600.0, y: 0.0 });
                }
                let definitions: Vec<_> = studio
                    .registry
                    .nodes
                    .values()
                    .map(|definition| {
                        (
                            definition.definition.id.clone(),
                            definition.definition.label.clone(),
                        )
                    })
                    .collect();
                for (id, label) in definitions {
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
                    let nodes = studio.workflow.graph.nodes.clone();
                    let edges = studio.workflow.graph.edges.clone();
                    let mut pins: BTreeMap<u64, (String, String, bool)> = BTreeMap::new();
                    let mut positions = Vec::new();
                    for node in &nodes {
                        let node_id = stable_id(&format!("node:{}", node.id));
                        if *frames < 2 {
                            ui.set_position(
                                node_id,
                                [node.position.x as f32, node.position.y as f32],
                            );
                        }
                        let (inputs, outputs) = node_ports(node, &studio.registry);
                        ui.node(node_id, |ui| {
                            ui.text(&node.data.label);
                            for (handle, label) in &inputs {
                                let pin = stable_id(&format!("pin:{}:{handle}", node.id));
                                pins.insert(pin, (node.id.clone(), handle.clone(), false));
                                ui.pin(pin, false, label);
                            }
                            for (handle, label) in &outputs {
                                let pin = stable_id(&format!("pin:{}:{handle}", node.id));
                                pins.insert(pin, (node.id.clone(), handle.clone(), true));
                                ui.pin(pin, true, label);
                            }
                        });
                        if ui.selected(node_id) {
                            *selected = node_id;
                        }
                        positions.push((node.id.clone(), ui.position(node_id)));
                    }
                    for edge in &edges {
                        let source =
                            stable_id(&format!("pin:{}:{}", edge.source, edge.source_handle));
                        let target =
                            stable_id(&format!("pin:{}:{}", edge.target, edge.target_handle));
                        ui.link(
                            stable_id(&format!("link:{}", edge.id)),
                            source,
                            target,
                            [0.3, 0.65, 0.9],
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
                    if let Some(link) = ui.deleted_link() {
                        if let Some(edge) = studio
                            .workflow
                            .graph
                            .edges
                            .iter()
                            .find(|edge| stable_id(&format!("link:{}", edge.id)) == link)
                            .cloned()
                        {
                            studio
                                .workflow
                                .graph
                                .edges
                                .retain(|candidate| candidate.id != edge.id);
                            studio.dirty = true;
                        }
                    }
                    for (id, position) in positions {
                        if let Some(node) = studio
                            .workflow
                            .graph
                            .nodes
                            .iter_mut()
                            .find(|node| node.id == id)
                        {
                            if *frames > 1 {
                                let changed = node.position.x != f64::from(position[0])
                                    || node.position.y != f64::from(position[1]);
                                node.position.x = f64::from(position[0]);
                                node.position.y = f64::from(position[1]);
                                studio.dirty |= changed;
                            }
                        }
                    }
                    if *frames == 2 {
                        ui.navigate();
                    }
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
                let params = node.data.params.clone();
                for (name, value) in params {
                    let changed = match value {
                        bite_schema::ParamValue::Number(number) => {
                            let mut edited = number as f32;
                            ui.drag_float(&name, &mut edited)
                                .then_some(bite_schema::ParamValue::Number(f64::from(edited)))
                        }
                        bite_schema::ParamValue::Int(number) => {
                            let mut edited = number as f32;
                            ui.drag_float(&name, &mut edited)
                                .then_some(bite_schema::ParamValue::Int(edited.round() as i64))
                        }
                        bite_schema::ParamValue::String(mut text) => ui
                            .input_text(&name, &mut text)
                            .then_some(bite_schema::ParamValue::String(text)),
                        bite_schema::ParamValue::Bool(value) => ui
                            .button(&format!("{name}: {value}"))
                            .then_some(bite_schema::ParamValue::Bool(!value)),
                        other => {
                            ui.text(&format!("{name}: {other:?}"));
                            None
                        }
                    };
                    if let Some(value) = changed {
                        studio.workflow.graph.nodes[index]
                            .data
                            .params
                            .insert(name, value);
                        studio.dirty = true;
                        studio.status = "Parameter changed; import again to refresh preview".into();
                    }
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
            let displayed: Vec<_> = if thumbnails.is_empty() {
                vec![2; 8]
            } else {
                thumbnails.clone()
            };
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
                        .with_title("BITE · Native technical prototype")
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
                event_loop.exit();
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
                }
                if event.state == ElementState::Pressed {
                    if let Some(text) = event.text {
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        s.demo.context.text_input(&text);
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => s.demo.context.text_input(&text),
            WindowEvent::Focused(f) => s.demo.context.focus(f),
            WindowEvent::DroppedFile(path) => {
                s.demo.dropped = path.display().to_string();
                if path.extension().and_then(|extension| extension.to_str()) == Some("bite") {
                    match Studio::open(s.demo.studio.registry.clone(), &path) {
                        Ok(studio) => {
                            s.demo.workflow_path = path.to_string_lossy().into_owned();
                            s.demo.studio = studio;
                            s.demo.frames = 0;
                        }
                        Err(error) => s.demo.studio.status = format!("Open failed: {error}"),
                    }
                } else if path.is_dir() {
                    s.demo.input_path = path.to_string_lossy().into_owned();
                    s.demo.studio.status = format!("Input folder: {}", path.display());
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {}
            _ => return,
        }
        s.settle = 2;
        s.window.request_redraw();
    }
}
fn smoke(path: &str, workflow: Option<&str>, import_folder: Option<&str>) -> Result<(), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    println!("Adapter: {:?}", adapter.get_info());
    let mut renderer =
        pollster::block_on(Renderer::new(&adapter, wgpu::TextureFormat::Rgba8Unorm))?;
    let mut demo = Demo::new(&mut renderer, 1.0, workflow.is_none())?;
    if let Some(workflow) = workflow {
        demo.studio = Studio::open(
            demo.studio.registry.clone(),
            PathBuf::from(workflow).as_path(),
        )?;
        demo.workflow_path = workflow.into();
        demo.frames = 0;
    }
    if let Some(import_folder) = import_folder {
        demo.input_path = import_folder.into();
        demo.import_images(&mut renderer)?;
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
        let data = demo.draw(width, height, 1., 1. / 60.);
        renderer.render(&view, &data, width, height, 1.);
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
