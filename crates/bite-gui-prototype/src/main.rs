//! Deliberately backend-independent Phase 8 test application.
mod renderer;
use bite_imgui::{Context, Key};
use renderer::Renderer;
use std::{sync::Arc, time::Instant};
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
}
impl Demo {
    fn new(renderer: &mut Renderer) -> Result<Self, String> {
        let mut context = Context::new()?;
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
        })
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
        } = self;
        let mut frame = context.frame(
            width as f32 / scale,
            height as f32 / scale,
            scale,
            delta.clamp(0.001, 0.1),
        );
        let mut ui = frame.ui();
        ui.dockspace();
        ui.window("Library", |ui| {
            ui.text("Ten fake node types");
            for name in [
                "Input", "Resize", "Color", "Mask", "Math", "Gate", "Merge", "Text", "Format",
                "Output",
            ] {
                ui.text(name);
            }
        });
        ui.window("Canvas", |ui| {
            ui.editor("Prototype", |ui| {
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
            });
        });
        ui.window("Inspector", |ui| {
            ui.text(&format!("Selected node: {selected}"));
            ui.drag_float("Value", value);
            ui.input_text("Text", text);
            ui.text(dropped);
        });
        ui.window("Preview", |ui| {
            ui.image(2, 256., 256.);
        });
        ui.window("Filmstrip", |ui| {
            for _ in 0..8 {
                ui.image(2, 64., 64.);
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
        let demo = Demo::new(&mut renderer)?;
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
            WindowEvent::DroppedFile(path) => s.demo.dropped = path.display().to_string(),
            WindowEvent::ScaleFactorChanged { .. } => {}
            _ => return,
        }
        s.settle = 2;
        s.window.request_redraw();
    }
}
fn smoke(path: &str) -> Result<(), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    println!("Adapter: {:?}", adapter.get_info());
    let mut renderer =
        pollster::block_on(Renderer::new(&adapter, wgpu::TextureFormat::Rgba8Unorm))?;
    let mut demo = Demo::new(&mut renderer)?;
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
    println!("Rendered 120-node prototype to {path}");
    Ok(())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--smoke") {
        return smoke(args.get(2).ok_or("--smoke needs a PNG output path")?);
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
