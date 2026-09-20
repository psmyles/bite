//! The window, the graphics surface and the event loop.
use crate::{app, commands, dialogs, logging, modals, persist, renderer::Renderer, work};
use bite_imgui::{Context, Key, MouseButton, Vec2};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

/// The event a background job sends to wake the loop.
#[derive(Debug, Clone, Copy)]
pub struct Wake;

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

struct State {
    surface: Surface,
    context: Context,
    editor: app::Editor,
    last_frame: Instant,
    /// A short burst of extra frames after input, so animations settle.
    settle: u8,
}

struct App {
    state: Option<State>,
    proxy: EventLoopProxy<Wake>,
    initial: Option<PathBuf>,
}

/// Starts the editor.
pub fn run(initial: Option<PathBuf>) -> Result<(), String> {
    logging::start_session();
    work::prune_cache();
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App {
        state: None,
        proxy,
        initial,
    };
    event_loop.run_app(&mut app).map_err(|error| error.to_string())
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.create(event_loop) {
            Ok(state) => self.state = Some(state),
            Err(error) => {
                logging::error(format!("Startup failed: {error}"));
                dialogs::message("Bite", &error);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: Wake) {
        if let Some(state) = &mut self.state {
            state.settle = state.settle.max(2);
            state.surface.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => {
                commands::run(&mut state.editor, crate::menu::Command::Exit);
                if state.editor.should_exit {
                    save_session(state);
                    event_loop.exit();
                    return;
                }
            }
            WindowEvent::Resized(size) => {
                state.surface.config.width = size.width.max(1);
                state.surface.config.height = size.height.max(1);
                state
                    .surface
                    .surface
                    .configure(&state.surface.renderer.device, &state.surface.config);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if state.context.set_scale(scale_factor as f32).is_ok() {
                    upload_font_atlas(state);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = state.surface.window.scale_factor() as f32;
                state
                    .context
                    .mouse_position(position.x as f32 / scale, position.y as f32 / scale);
            }
            WindowEvent::MouseInput { state: pressed, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => Some(MouseButton::Left),
                    winit::event::MouseButton::Right => Some(MouseButton::Right),
                    winit::event::MouseButton::Middle => Some(MouseButton::Middle),
                    _ => None,
                };
                if let Some(button) = button {
                    state
                        .context
                        .mouse_button(button, pressed == ElementState::Pressed);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as f32 / 40.0, position.y as f32 / 40.0)
                    }
                };
                state.context.mouse_wheel(x, y);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(key) = map_key(code) {
                        state.context.key(key, pressed);
                    }
                }
                if pressed {
                    if let Some(text) = event.text.as_ref() {
                        // Control characters are handled as keys, never as text.
                        let printable: String = text
                            .chars()
                            .filter(|character| !character.is_control())
                            .collect();
                        if !printable.is_empty() {
                            state.context.text_input(&printable);
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let modifiers = modifiers.state();
                state.context.key(Key::Ctrl, modifiers.control_key());
                state.context.key(Key::Shift, modifiers.shift_key());
                state.context.key(Key::Alt, modifiers.alt_key());
                state.context.key(Key::Super, modifiers.super_key());
            }
            WindowEvent::Focused(focused) => state.context.focus(focused),
            WindowEvent::DroppedFile(path) => handle_drop(state, path),
            WindowEvent::RedrawRequested => {
                self.draw(event_loop);
                return;
            }
            _ => {}
        }
        if let Some(state) = &mut self.state {
            state.settle = 2;
            state.surface.window.request_redraw();
        }
    }
}

impl App {
    fn create(&mut self, event_loop: &ActiveEventLoop) -> Result<State, String> {
        let mut editor = app::Editor::new()?;
        let session = persist::Session::load();
        let monitors: Vec<(i32, i32, u32, u32)> = event_loop
            .available_monitors()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (position.x, position.y, size.width, size.height)
            })
            .collect();
        let bounds = session.window_within(&monitors);

        let attributes = Window::default_attributes()
            .with_title("Untitled - Bite")
            .with_inner_size(winit::dpi::LogicalSize::new(bounds.width, bounds.height))
            .with_position(winit::dpi::LogicalPosition::new(bounds.x, bounds.y));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| error.to_string())?,
        );
        window.set_ime_allowed(true);
        if bounds.maximized {
            window.set_maximized(true);
        }

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| error.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        // A non-sRGB target keeps the interface colors exactly as authored.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(capabilities.formats[0]);
        let renderer = pollster::block_on(Renderer::new(&adapter, format))?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: Vec::new(),
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        surface.configure(&renderer.device, &config);

        let scale = window.scale_factor() as f32;
        let context = Context::new(scale)?;
        crate::theme::apply_base_style();

        let proxy = self.proxy.clone();
        editor.jobs.set_waker(Arc::new(move || {
            let _ = proxy.send_event(Wake);
        }));
        editor.session = session;
        editor.sync_active_input();

        let mut state = State {
            surface: Surface {
                window,
                surface,
                config,
                renderer,
            },
            context,
            editor,
            last_frame: Instant::now(),
            settle: 8,
        };
        upload_font_atlas(&mut state);

        if let Some(path) = self.initial.take() {
            commands::open_path(&mut state.editor, &path);
        }
        // The startup check reports only when a newer release exists.
        let handle = state.editor.jobs.handle();
        std::thread::spawn(move || {
            if let Ok(info) = crate::updates::check() {
                if info.available {
                    handle.send(work::Message::UpdateChecked(Box::new(Ok(info))));
                }
            }
        });
        logging::info("Editor started");
        Ok(state)
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };

        // Background results are applied before the frame that shows them.
        while let Some(message) = state.editor.jobs.try_recv() {
            commands::apply_message(&mut state.editor, message);
        }
        upload_pending(state);
        if state.editor.jobs.take_preview_request() {
            refresh_preview(state);
        }
        update_progress(state);

        let now = Instant::now();
        let delta = now.duration_since(state.last_frame).as_secs_f32();
        state.last_frame = now;

        let scale = state.surface.window.scale_factor() as f32;
        let size: Vec2 = [
            state.surface.config.width as f32 / scale,
            state.surface.config.height as f32 / scale,
        ];

        let data = app::draw_frame(&mut state.editor, &mut state.context, size, scale, delta);

        state
            .surface
            .window
            .set_cursor(app::cursor_for(state.context.mouse_cursor()));
        let title = state.editor.title();
        state.surface.window.set_title(&title);
        let fullscreen = state.editor.fullscreen;
        let is_fullscreen = state.surface.window.fullscreen().is_some();
        if fullscreen != is_fullscreen {
            state
                .surface
                .window
                .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }

        match state.surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame.texture.create_view(&Default::default());
                state.surface.renderer.render(
                    &view,
                    &data,
                    state.surface.config.width,
                    state.surface.config.height,
                    scale,
                );
                state.surface.window.pre_present_notify();
                state.surface.renderer.queue.present(frame);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                state
                    .surface
                    .surface
                    .configure(&state.surface.renderer.device, &state.surface.config);
                state.surface.window.request_redraw();
            }
            // A timed out or hidden surface simply skips this frame.
            _ => {}
        }

        if state.editor.should_exit {
            save_session(state);
            event_loop.exit();
            return;
        }
        // A short burst keeps hover states and caret blinking responsive.
        if state.settle > 0 {
            state.settle -= 1;
            state.surface.window.request_redraw();
        } else if state.editor.run.is_some()
            || matches!(
                state.editor.modal,
                modals::Modal::BatchProgress | modals::Modal::ImportProgress
            )
        {
            // Progress dialogs tick their elapsed time once a second.
            state.surface.window.request_redraw();
        }
    }
}

/// Uploads the font atlas and tells the interface which texture holds it.
fn upload_font_atlas(state: &mut State) {
    let (width, height, pixels) = state.context.fonts().texture();
    state.surface.renderer.texture(1, width, height, &pixels);
    state.context.fonts().set_texture_id(1);
}

/// Uploads whatever decoded images arrived since the last frame.
fn upload_pending(state: &mut State) {
    let pending = std::mem::take(&mut state.editor.pending_uploads);
    let had_thumbnails = pending
        .iter()
        .any(|(key, _)| key.starts_with("thumbnail:"));
    for (key, image) in pending {
        if image.width == 0 || image.height == 0 {
            continue;
        }
        let id = app::texture_id(&key);
        state
            .surface
            .renderer
            .texture(id, image.width, image.height, &image.pixels);
        if !state.editor.textures.contains(&id) {
            state.editor.textures.push(id);
        }
        if let Some(rest) = key.strip_prefix("thumbnail:") {
            if let Some((node, index)) = rest.rsplit_once(':') {
                if let (Some(branch), Ok(index)) =
                    (state.editor.branches.get_mut(node), index.parse::<usize>())
                {
                    if let Some(thumbnail) = branch.thumbnails.get_mut(index) {
                        thumbnail.texture = Some(id);
                    }
                }
            }
        } else if let Some(rest) = key.strip_prefix("preview:") {
            if let Some((node, _)) = rest.rsplit_once(':') {
                let name = state
                    .editor
                    .branches
                    .get(node)
                    .and_then(|branch| {
                        branch
                            .selected
                            .and_then(|index| branch.paths.get(index).cloned())
                    })
                    .map(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_string()
                    })
                    .unwrap_or_default();
                let format = name
                    .rsplit('.')
                    .next()
                    .filter(|extension| *extension != name.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(branch) = state.editor.branches.get_mut(node) {
                    let source = branch.preview_source;
                    branch.preview = Some(crate::panels::preview::PreviewImage {
                        texture: Some(id),
                        width: source.width.max(1),
                        height: source.height.max(1),
                        name,
                        format,
                        bytes: source.bytes,
                        pixels: [image.width, image.height],
                    });
                }
            }
        }
    }
    if had_thumbnails {
        // The strip now has a texture to stand in with until the first render lands.
        state.editor.seed_preview_from_thumbnail();
    }
}

/// Hands the selected image to the preview worker.
///
/// With nothing to process the panel shows the thumbnail the filmstrip already holds, which
/// is the image Electron's pipeline returns when the graph has no nodes. That costs nothing
/// and switches with the pointer; anything with a chain in it goes to the worker.
fn refresh_preview(state: &mut State) {
    let editor = &mut state.editor;
    let Some(node) = editor.active_input.clone() else {
        return;
    };
    let Some(branch) = editor.branches.get(&node) else {
        return;
    };
    let Some(index) = branch.selected else { return };
    let Some(path) = branch.paths.get(index).cloned() else {
        return;
    };
    let Some(target) = editor.effective_preview_node() else {
        editor.show_thumbnail_as_preview();
        return;
    };
    editor.jobs.submit_preview(work::PreviewJob {
        graph: editor.studio.workflow.graph.clone(),
        registry: Arc::new(editor.studio.registry.clone()),
        path,
        node: node.clone(),
        index,
        thumbnail_size: commands::thumbnail_size(editor, &node),
        target,
    });
}

/// Keeps the elapsed time on the progress dialogs moving.
fn update_progress(state: &mut State) {
    if let Some(run) = &state.editor.run {
        state.editor.progress.elapsed_seconds = run.started.elapsed().as_secs_f32();
    }
    if let Some(started) = state.editor.import_started {
        state.editor.progress.elapsed_seconds = started.elapsed().as_secs_f32();
    }
}

/// Handles a file dropped on the window.
fn handle_drop(state: &mut State, path: PathBuf) {
    let editor = &mut state.editor;
    if path.extension().and_then(|extension| extension.to_str()) == Some("bite") {
        editor.pending_open = Some(path);
        if editor.studio.dirty {
            editor.modal = modals::Modal::Confirm {
                message: modals::confirm_message(modals::PendingAction::OpenPath),
                pending: modals::PendingAction::OpenPath,
            };
        } else {
            commands::perform_pending(editor, modals::PendingAction::OpenPath);
        }
        return;
    }
    let Some(node) = editor.active_input.clone() else {
        return;
    };
    if path.is_dir() {
        editor.inspector.scan_folder = path.to_string_lossy().into_owned();
        return;
    }
    if dialogs::is_image_path(&path) {
        commands::add_paths(editor, &node, vec![path]);
    }
}

/// Stores the window bounds and panel sizes for the next launch.
fn save_session(state: &mut State) {
    let window = &state.surface.window;
    let size = window.inner_size();
    let scale = window.scale_factor();
    let position = window
        .outer_position()
        .map(|position| (position.x, position.y))
        .unwrap_or((80, 60));
    state.editor.session.window = persist::WindowBounds {
        x: position.0,
        y: position.1,
        width: (f64::from(size.width) / scale) as u32,
        height: (f64::from(size.height) / scale) as u32,
        maximized: window.is_maximized(),
    };
    state.editor.session.save();
    logging::info("Editor closed");
}

/// Maps a physical key to the interface's key set.
fn map_key(code: KeyCode) -> Option<Key> {
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
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        KeyCode::Equal => Key::Equal,
        KeyCode::NumpadAdd => Key::Plus,
        KeyCode::Digit0 | KeyCode::Numpad0 => Key::Digit0,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        _ => return None,
    })
}

/// The interval the progress dialogs redraw at while work is in flight.
pub const PROGRESS_TICK: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_map_covers_the_editing_shortcuts() {
        assert_eq!(map_key(KeyCode::KeyZ), Some(Key::Z));
        assert_eq!(map_key(KeyCode::Delete), Some(Key::Delete));
        assert_eq!(map_key(KeyCode::Space), Some(Key::Space));
        assert_eq!(map_key(KeyCode::F11), Some(Key::F11));
        assert_eq!(map_key(KeyCode::F5), None);
    }

    #[test]
    fn both_enter_keys_map_to_the_same_action() {
        assert_eq!(map_key(KeyCode::Enter), map_key(KeyCode::NumpadEnter));
    }
}
