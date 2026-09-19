//! Small owned wrapper. No BITE workflow or pipeline dependencies.
use bite_imgui_sys as sys;
use std::{
    cell::Cell,
    ffi::CString,
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, MutexGuard},
};
static OWNER: Mutex<()> = Mutex::new(());
fn c(text: &str) -> CString {
    CString::new(text.replace('\0', "�")).unwrap()
}
fn ui_font_path() -> Option<PathBuf> {
    let configured = std::env::var_os("BITE_UI_FONT").map(PathBuf::from);
    if configured.as_ref().is_some_and(|path| path.is_file()) {
        return configured;
    }

    #[cfg(target_os = "windows")]
    let candidates = {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let windows = std::env::var_os("WINDIR").map(PathBuf::from);
        [
            local
                .as_ref()
                .map(|root| root.join("Microsoft/Windows/Fonts/Inter.ttc")),
            windows.as_ref().map(|root| root.join("Fonts/Inter.ttc")),
            windows.as_ref().map(|root| root.join("Fonts/Inter.ttf")),
        ]
    };
    #[cfg(target_os = "macos")]
    let candidates = [
        Some(PathBuf::from("/Library/Fonts/Inter.ttc")),
        Some(PathBuf::from("/Library/Fonts/Inter.ttf")),
    ];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let candidates = [
        Some(PathBuf::from("/usr/share/fonts/truetype/inter/Inter.ttc")),
        Some(PathBuf::from(
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
        )),
    ];

    candidates.into_iter().flatten().find(|path| path.is_file())
}
pub struct Context {
    raw: *mut std::ffi::c_void,
    _owner: MutexGuard<'static, ()>,
    _thread: PhantomData<Rc<()>>,
}
pub struct Frame<'a> {
    context: &'a mut Context,
    rendered: bool,
}
pub struct Ui<'a> {
    raw: *mut std::ffi::c_void,
    scope: Rc<Cell<u8>>,
    _frame: PhantomData<&'a mut ()>,
}
#[derive(Clone, Debug)]
pub struct DrawList {
    pub vertices: Vec<sys::BiteVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<sys::BiteCommand>,
}
pub type DrawData = Vec<DrawList>;
#[derive(Clone, Copy)]
pub enum Key {
    Tab,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    Backspace,
    Space,
    Enter,
    Escape,
    Ctrl,
    Shift,
    Alt,
    Super,
    A,
    C,
    V,
    X,
    Y,
    Z,
    F,
}
impl Context {
    pub fn new() -> Result<Self, String> {
        Self::with_scale(1.0)
    }
    pub fn with_scale(scale: f32) -> Result<Self, String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("UI scale must be a positive finite number".into());
        }
        let owner = OWNER
            .try_lock()
            .map_err(|_| "only one ImGui context may be active")?;
        let font = ui_font_path()
            .and_then(|path| path.to_str().map(c))
            .unwrap_or_else(|| c(""));
        let raw = unsafe { sys::bite_create(font.as_ptr(), 16.0, scale) };
        if raw.is_null() {
            return Err("ImGui context creation failed".into());
        }
        let mut context = Self {
            raw,
            _owner: owner,
            _thread: PhantomData,
        };
        context.font_atlas();
        Ok(context)
    }
    pub fn font_atlas(&mut self) -> (u32, u32, Vec<u8>) {
        let mut pixels = std::ptr::null();
        let (mut w, mut h) = (0, 0);
        unsafe {
            sys::bite_font_pixels(&mut pixels, &mut w, &mut h);
            (
                w as u32,
                h as u32,
                std::slice::from_raw_parts(pixels, w as usize * h as usize * 4).to_vec(),
            )
        }
    }
    pub fn set_font_texture(&mut self, id: u64) {
        unsafe { sys::bite_font_texture(id) }
    }
    pub fn mouse_position(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::bite_mouse_position(x, y) }
        }
    }
    pub fn mouse_button(&mut self, button: u8, down: bool) {
        if button < 5 {
            unsafe { sys::bite_mouse_button(button.into(), down.into()) }
        }
    }
    pub fn mouse_wheel(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::bite_mouse_wheel(x, y) }
        }
    }
    pub fn key(&mut self, key: Key, down: bool) {
        let code = match key {
            Key::Tab => 512,
            Key::Left => 513,
            Key::Right => 514,
            Key::Up => 515,
            Key::Down => 516,
            Key::PageUp => 517,
            Key::PageDown => 518,
            Key::Home => 519,
            Key::End => 520,
            Key::Insert => 521,
            Key::Delete => 522,
            Key::Backspace => 523,
            Key::Space => 524,
            Key::Enter => 525,
            Key::Escape => 526,
            Key::Ctrl => 4096,
            Key::Shift => 8192,
            Key::Alt => 16384,
            Key::Super => 32768,
            Key::A => 546,
            Key::C => 548,
            Key::F => 551,
            Key::V => 567,
            Key::X => 569,
            Key::Y => 570,
            Key::Z => 571,
        };
        unsafe { sys::bite_key(code, down.into()) }
    }
    pub fn text_input(&mut self, text: &str) {
        unsafe { sys::bite_text_input(c(text).as_ptr()) }
    }
    pub fn focus(&mut self, focused: bool) {
        unsafe { sys::bite_focus(focused.into()) }
    }
    pub fn frame(&mut self, width: f32, height: f32, scale: f32, delta: f32) -> Frame<'_> {
        assert!(
            width.is_finite()
                && height.is_finite()
                && scale.is_finite()
                && delta.is_finite()
                && width > 0.0
                && height > 0.0
                && scale > 0.0
                && delta > 0.0
        );
        unsafe {
            sys::bite_frame(width, height, scale, delta);
        }
        Frame {
            context: self,
            rendered: false,
        }
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sys::bite_destroy(self.raw) }
    }
}
impl Frame<'_> {
    pub fn ui(&mut self) -> Ui<'_> {
        Ui {
            raw: self.context.raw,
            scope: Rc::new(Cell::new(0)),
            _frame: PhantomData,
        }
    }
    pub fn render(mut self) -> DrawData {
        unsafe {
            sys::bite_render();
        }
        self.rendered = true;
        let mut data = Vec::new();
        unsafe {
            for i in 0..sys::bite_draw_list_count() {
                let mut vertices =
                    vec![sys::BiteVertex::default(); sys::bite_vertex_count(i) as usize];
                let mut indices = vec![0; sys::bite_index_count(i) as usize];
                sys::bite_copy_vertices(i, vertices.as_mut_ptr());
                sys::bite_copy_indices(i, indices.as_mut_ptr());
                let mut commands = Vec::new();
                for j in 0..sys::bite_command_count(i) {
                    let mut command = sys::BiteCommand::default();
                    if sys::bite_get_command(i, j, &mut command) != 0 {
                        commands.push(command);
                    }
                }
                data.push(DrawList {
                    vertices,
                    indices,
                    commands,
                });
            }
        }
        data
    }
}
impl Drop for Frame<'_> {
    fn drop(&mut self) {
        if !self.rendered {
            unsafe {
                sys::bite_render();
            }
        }
    }
}
struct Scope(unsafe extern "C" fn());
impl Drop for Scope {
    fn drop(&mut self) {
        unsafe { (self.0)() }
    }
}
struct StateScope(Rc<Cell<u8>>, u8);
impl Drop for StateScope {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}
impl Ui<'_> {
    pub fn dockspace(&mut self) {
        assert_eq!(self.scope.get(), 0, "dockspace cannot be inside an editor");
        unsafe { sys::bite_dockspace() }
    }
    pub fn window(&mut self, title: &str, body: impl FnOnce(&mut Self)) {
        assert_eq!(self.scope.get(), 0, "window cannot be inside an editor");
        let visible = unsafe { sys::bite_begin(c(title).as_ptr()) != 0 };
        let _scope = Scope(sys::bite_end);
        if visible {
            body(self);
        }
    }
    pub fn text(&mut self, text: &str) {
        unsafe { sys::bite_text(c(text).as_ptr()) }
    }
    pub fn button(&mut self, text: &str) -> bool {
        unsafe { sys::bite_button(c(text).as_ptr()) != 0 }
    }
    pub fn drag_float(&mut self, label: &str, value: &mut f32) -> bool {
        unsafe { sys::bite_drag_float(c(label).as_ptr(), value) != 0 }
    }
    pub fn input_text(&mut self, label: &str, text: &mut String) -> bool {
        let mut bytes = vec![0u8; text.len().max(4095) + 1];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        let changed = unsafe {
            sys::bite_input_text(c(label).as_ptr(), bytes.as_mut_ptr().cast(), bytes.len()) != 0
        };
        if changed {
            let len = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            *text = String::from_utf8_lossy(&bytes[..len]).into_owned();
        }
        changed
    }
    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> bool {
        let mut raw = i32::from(*value);
        let changed = unsafe { sys::bite_checkbox(c(label).as_ptr(), &mut raw) != 0 };
        *value = raw != 0;
        changed
    }
    pub fn combo(&mut self, label: &str, current: &mut usize, items: &[String]) -> bool {
        if items.is_empty() {
            return false;
        }
        let mut encoded = Vec::new();
        for item in items {
            encoded.extend(item.bytes().filter(|byte| *byte != 0));
            encoded.push(0);
        }
        encoded.push(0);
        let mut selected = (*current).min(items.len() - 1) as i32;
        let changed = unsafe {
            sys::bite_combo(c(label).as_ptr(), &mut selected, encoded.as_ptr().cast()) != 0
        };
        *current = (selected as usize).min(items.len() - 1);
        changed
    }
    pub fn next_item_full_width(&mut self) {
        unsafe { sys::bite_next_item_full_width() }
    }
    pub fn image(&mut self, id: u64, width: f32, height: f32) {
        unsafe { sys::bite_image(id, width, height) }
    }
    pub fn image_button(&mut self, label: &str, id: u64, width: f32, height: f32) -> bool {
        unsafe { sys::bite_image_button(c(label).as_ptr(), id, width, height) != 0 }
    }
    pub fn same_line(&mut self) {
        unsafe { sys::bite_same_line() }
    }
    pub fn editor(&mut self, label: &str, body: impl FnOnce(&mut Self)) {
        assert_eq!(self.scope.get(), 0, "editors cannot be nested");
        let _state = StateScope(self.scope.clone(), self.scope.replace(1));
        unsafe {
            sys::bite_editor_begin(self.raw, c(label).as_ptr());
        }
        let _scope = Scope(sys::bite_editor_end);
        body(self);
    }
    pub fn node(&mut self, id: u64, body: impl FnOnce(&mut Self)) {
        assert_eq!(
            self.scope.get(),
            1,
            "node requires an editor and cannot be nested"
        );
        let _state = StateScope(self.scope.clone(), self.scope.replace(2));
        assert_ne!(id, 0);
        unsafe {
            sys::bite_node_begin(id);
        }
        let _scope = Scope(sys::bite_node_end);
        body(self);
    }
    pub fn pin(&mut self, id: u64, output: bool, label: &str) {
        assert_eq!(self.scope.get(), 2, "pin requires a node");
        assert_ne!(id, 0);
        unsafe {
            sys::bite_pin_begin(id, output.into());
            sys::bite_text(c(label).as_ptr());
            sys::bite_pin_end();
        }
    }
    pub fn link(&mut self, id: u64, source: u64, target: u64, color: [f32; 3]) {
        assert_eq!(self.scope.get(), 1, "link requires an editor");
        assert!(id != 0 && source != 0 && target != 0 && color.iter().all(|n| n.is_finite()));
        unsafe {
            sys::bite_link(id, source, target, color[0], color[1], color[2]);
        }
    }
    pub fn new_link(&mut self) -> Option<(u64, u64)> {
        assert_eq!(self.scope.get(), 1, "link creation requires an editor");
        let (mut a, mut b) = (0, 0);
        (unsafe { sys::bite_new_link(&mut a, &mut b) } != 0).then_some((a, b))
    }
    pub fn new_node(&mut self) -> Option<u64> {
        assert_eq!(self.scope.get(), 1, "node creation requires an editor");
        let mut pin = 0;
        (unsafe { sys::bite_new_node(&mut pin) } != 0).then_some(pin)
    }
    pub fn deleted_link(&mut self) -> Option<u64> {
        assert_eq!(self.scope.get(), 1, "link deletion requires an editor");
        let mut id = 0;
        (unsafe { sys::bite_deleted_link(&mut id) } != 0).then_some(id)
    }
    pub fn position(&mut self, id: u64) -> [f32; 2] {
        assert!(
            self.scope.get() > 0 && id != 0,
            "position requires an editor"
        );
        let (mut x, mut y) = (0.0, 0.0);
        unsafe {
            sys::bite_get_node_position(id, &mut x, &mut y);
        }
        [x, y]
    }
    pub fn set_position(&mut self, id: u64, p: [f32; 2]) {
        assert!(
            self.scope.get() > 0 && id != 0 && p.iter().all(|n| n.is_finite()),
            "position requires an editor"
        );
        unsafe {
            sys::bite_set_node_position(id, p[0], p[1]);
        }
    }
    pub fn selected(&mut self, id: u64) -> bool {
        assert!(
            self.scope.get() > 0 && id != 0,
            "selection requires an editor"
        );
        unsafe { sys::bite_node_selected(id) != 0 }
    }
    pub fn group(&mut self, width: f32, height: f32) {
        assert_eq!(self.scope.get(), 2, "group requires a node");
        assert!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0);
        unsafe {
            sys::bite_group(width, height);
        }
    }
    pub fn background_menu(&mut self) -> bool {
        assert_eq!(self.scope.get(), 1, "background menu requires an editor");
        unsafe { sys::bite_background_menu() != 0 }
    }
    pub fn navigate(&mut self) {
        assert_eq!(self.scope.get(), 1, "navigation requires an editor");
        unsafe { sys::bite_navigate() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_draw_data_survives_context() {
        let data = {
            let mut context = Context::new().unwrap();
            let (w, h, pixels) = context.font_atlas();
            assert_eq!(pixels.len(), (w * h * 4) as usize);
            context.set_font_texture(1);
            let mut data = Vec::new();
            for _ in 0..3 {
                let mut frame = context.frame(1280.0, 720.0, 1.0, 1.0 / 60.0);
                frame.ui().window("Canvas", |ui| {
                    ui.editor("editor", |ui| {
                        ui.node(1, |ui| {
                            ui.text("Node");
                            ui.pin(2, false, "Input");
                            ui.pin(3, true, "Output");
                        });
                    })
                });
                data = frame.render();
            }
            data
        };
        assert!(data.iter().any(|l| !l.vertices.is_empty()));
    }
}
