//! The window, the swapchain and the event loop.
//!
//! The shell owns the GPU: it creates the device and the swapchain, and hands sokol_gfx the first
//! at `sg::setup` and a render-target view of the second per frame. sokol_gfx never touches a
//! window, and nothing above this module names a graphics API at all.
//!
//! **The device comes up on its own thread, started before the event loop exists.** It is the
//! single longest item on the launch path, and it needs no window - so it runs alongside the
//! logging, the cache prune, `Editor::new` and the window creation, which together are most of
//! what happens before the first frame. The join is the hand-off: D3D11 devices are free-threaded
//! and sokol_gfx has no thread affinity, only a single-user rule. The one per-window piece, the
//! swapchain, is a couple of milliseconds on the main thread afterwards.
use crate::{
    app, commands, dialogs, icon, logging, modals, persist,
    render::{self, Device, SWAPCHAIN_FORMAT, Swapchain, Textures},
    theme, timing, work,
};
use bite_imgui::{Context, Key, MouseButton, Vec2};
use sokol::gfx as sg;
use std::{
    path::PathBuf,
    sync::Arc,
    thread::JoinHandle,
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
    swapchain: Swapchain,
    /// The images the editor uploaded: thumbnails and previews. Dear ImGui's own textures - the
    /// font atlas - are sokol_imgui's and never appear here.
    textures: Textures,
}

struct State {
    surface: Surface,
    context: Context,
    editor: app::Editor,
    last_frame: Instant,
    /// A short burst of extra frames after input, so animations settle.
    settle: u8,
    /// Kept only so the device outlives sokol_gfx: `sg::shutdown` runs against it.
    _device: Device,
}

struct App {
    state: Option<State>,
    proxy: EventLoopProxy<Wake>,
    initial: Option<PathBuf>,
    /// The GPU bring-up, started at the top of [`run`] and joined once the window exists.
    gpu: Option<JoinHandle<Result<Device, String>>>,
}

/// Starts the editor.
pub fn run(initial: Option<PathBuf>) -> Result<(), String> {
    timing::report("run()");
    // Before anything else: everything below this line runs while the GPU comes up.
    let gpu = start_gpu();
    logging::start_session();
    // Walking the thumbnail cache and deleting stale files is housekeeping nothing waits on, so
    // it does not belong on the path to the first frame - it tolerates a missing directory and
    // reports only to the log. It is a fraction of a millisecond here and the startup is now
    // bounded by the GPU anyway, so this buys no time; it is off the path because that is where
    // it belongs, and because a cache large enough to matter is exactly when it would.
    std::thread::Builder::new()
        .name("bite-cache-prune".into())
        .spawn(work::prune_cache)
        .map_err(|error| error.to_string())?;
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    timing::report("event loop built");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App {
        state: None,
        proxy,
        initial,
        gpu: Some(gpu),
    };
    event_loop.run_app(&mut app).map_err(|error| error.to_string())
}

/// Brings the GPU up on a worker thread: the device, sokol_gfx on it, and sokol_imgui on that.
///
/// None of the three needs a window, and together they are the longest single item on the launch
/// path. `simgui_setup` belongs here too - it needs sokol_gfx, not a context - and it is followed
/// immediately by the call that clears the context it leaves current, because nothing may touch
/// Dear ImGui between the two.
fn start_gpu() -> JoinHandle<Result<Device, String>> {
    std::thread::Builder::new()
        .name("bite-gpu-init".into())
        .spawn(|| {
            let device = Device::create(std::env::var_os("BITE_GPU").is_some_and(|v| v == "warp"))?;
            timing::report("gpu device");

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
            if !sg::isvalid() {
                return Err("sokol_gfx could not be set up on the graphics device".into());
            }
            // SAFETY: sokol_gfx is up, no ImGui frame is open, and no context exists yet - the
            // main thread makes the one context after joining this thread.
            unsafe { render::imgui::setup_once(SWAPCHAIN_FORMAT) };
            timing::report("gpu ready (sokol_gfx + sokol_imgui)");
            Ok(device)
        })
        .expect("the GPU bring-up thread must start")
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        timing::report("resumed");
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
                // A zero dimension is a minimised window. The swapchain remembers it and the
                // frame is skipped; DXGI refuses to resize to it.
                state.surface.swapchain.resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let _ = state.context.set_scale(scale_factor as f32);
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
            // Moving the window needs no new frame: the compositor carries the one already
            // presented. Drawing on every move instead put a frame that waits for the
            // vertical blank inside the drag loop, which is what made dragging stutter.
            WindowEvent::Moved(_) => return,
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
        timing::report("Editor::new");
        // `Editor::new` has already read the session; reading it again here - which this used to
        // do - parsed the same file twice and then threw the first copy away.
        let session = editor.session.clone();
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
            .with_window_icon(icon::window_icon())
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
        timing::report("window created");

        // Everything above ran alongside the bring-up; this is where the two paths meet.
        let device = self
            .gpu
            .take()
            .ok_or("the GPU bring-up was already taken")?
            .join()
            .map_err(|_| "the GPU bring-up thread panicked".to_string())??;
        timing::report("gpu joined");

        let size = window.inner_size();
        let swapchain = Swapchain::new(&device, &window, size.width, size.height)?;
        timing::report("swapchain");

        let scale = window.scale_factor() as f32;
        let context = Context::new(scale)?;
        timing::report("imgui Context (fonts)");
        theme::apply_base_style();

        let proxy = self.proxy.clone();
        editor.jobs.set_waker(Arc::new(move || {
            let _ = proxy.send_event(Wake);
        }));
        editor.sync_active_input();

        let mut state = State {
            surface: Surface {
                window,
                swapchain,
                textures: Textures::default(),
            },
            context,
            editor,
            last_frame: Instant::now(),
            settle: 8,
            _device: device,
        };
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

        // The render target is acquired *before* the frame is built, not after.
        //
        // A minimised window has a zero-size client and a device that has gone away has nothing
        // to draw into; either way there is no frame. But sokol_imgui is what *ends* an ImGui
        // frame - `app::draw_frame` only hands it over - so building one and then returning
        // without rendering would leave the context open, and the next `igNewFrame` asserts. The
        // window survived being minimised and died on the way back; this order is why it does
        // not. `scripts/window-exercise.ps1` is what found it and what checks it.
        let (width, height) = state.surface.swapchain.size();
        let mut swapchain = sg::Swapchain::new();
        swapchain.width = width as i32;
        swapchain.height = height as i32;
        swapchain.sample_count = 1;
        swapchain.color_format = SWAPCHAIN_FORMAT;
        swapchain.depth_format = sg::PixelFormat::None;
        if !state.surface.swapchain.acquire(&mut swapchain) {
            if state.editor.should_exit {
                save_session(state);
                event_loop.exit();
            }
            return;
        }

        // Only now does time advance: a skipped frame must not be charged to the next one's
        // delta, or an animation jumps when the window comes back.
        let now = Instant::now();
        let delta = now.duration_since(state.last_frame).as_secs_f32();
        state.last_frame = now;

        let scale = state.surface.window.scale_factor() as f32;
        let size: Vec2 = [width as f32 / scale, height as f32 / scale];

        app::draw_frame(&mut state.editor, &mut state.context, size, scale, delta);

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

        let mut pass = sg::Pass::new();
        pass.swapchain = swapchain;
        // The shell gap colour, so a resize never flashes an unpainted edge.
        pass.action.colors[0] = sg::ColorAttachmentAction {
            load_action: sg::LoadAction::Clear,
            store_action: sg::StoreAction::Store,
            clear_value: sg::Color {
                r: theme::GAP_COLOR.0[0],
                g: theme::GAP_COLOR.0[1],
                b: theme::GAP_COLOR.0[2],
                a: 1.0,
            },
        };
        sg::begin_pass(&pass);
        // SAFETY: a frame is open on the current context and a pass is open, which is what
        // `render` requires. It is `igRender` plus the texture uploads plus the draw calls, so
        // the glyphs this frame was the first to show reach the device here too.
        unsafe { render::imgui::render() };
        sg::end_pass();
        sg::commit();
        state.surface.window.pre_present_notify();
        state.surface.swapchain.present();
        timing::stamp_first_frame();

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

/// Uploads whatever decoded images arrived since the last frame.
///
/// Two identifiers are in play and they are not the same number. The *key* is what the editor
/// computes from the cache name (`app::texture_id`) and what it frees by; the *draw id* is what
/// sokol_imgui turns back into a view when it meets the draw command, and it is what goes into a
/// thumbnail or a preview. Before the sokol shell those were one number, because the renderer's
/// texture map was keyed on the editor's own id.
fn upload_pending(state: &mut State) {
    let pending = std::mem::take(&mut state.editor.pending_uploads);
    let had_thumbnails = pending
        .iter()
        .any(|(key, _)| key.starts_with("thumbnail:"));
    for (key, image) in pending {
        if image.width == 0 || image.height == 0 {
            continue;
        }
        let key_id = app::texture_id(&key);
        let id = state
            .surface
            .textures
            .upload(key_id, image.width, image.height, &image.pixels);
        if id == 0 {
            continue;
        }
        if !state.editor.textures.contains(&key_id) {
            state.editor.textures.push(key_id);
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
/// and switches with the pointer, while the worker still reports what the value nodes read
/// from the file, as the Electron preview did for a workflow it had no chain to follow.
fn refresh_preview(state: &mut State) {
    let editor = &mut state.editor;
    let selected = editor.active_input.clone().and_then(|node| {
        let branch = editor.branches.get(&node)?;
        let index = branch.selected?;
        let path = branch.paths.get(index).cloned()?;
        Some((node, index, path))
    });
    let Some((node, index, path)) = selected else {
        editor.resolve_values();
        return;
    };
    // With no chain to render, the panel shows the image itself while the job still
    // reports what the value nodes read from it.
    let target = editor.effective_preview_node();
    if target.is_none() {
        editor.show_thumbnail_as_preview();
    }
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
