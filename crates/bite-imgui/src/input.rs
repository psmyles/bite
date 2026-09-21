//! Keyboard, pointer and cursor types, plus the frame-scoped input queries.
use crate::{Ui, Vec2, from_v};
use bite_imgui_sys as sys;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
}

/// The cursor shape the interface requests; the host window applies it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseCursor {
    Arrow,
    TextInput,
    ResizeAll,
    ResizeNorthSouth,
    ResizeEastWest,
    ResizeNorthEastSouthWest,
    ResizeNorthWestSouthEast,
    Hand,
    NotAllowed,
}

impl MouseCursor {
    pub(crate) fn from_raw(value: sys::ImGuiMouseCursor) -> Self {
        match value {
            sys::ImGuiMouseCursor_TextInput => Self::TextInput,
            sys::ImGuiMouseCursor_ResizeAll => Self::ResizeAll,
            sys::ImGuiMouseCursor_ResizeNS => Self::ResizeNorthSouth,
            sys::ImGuiMouseCursor_ResizeEW => Self::ResizeEastWest,
            sys::ImGuiMouseCursor_ResizeNESW => Self::ResizeNorthEastSouthWest,
            sys::ImGuiMouseCursor_ResizeNWSE => Self::ResizeNorthWestSouthEast,
            sys::ImGuiMouseCursor_Hand => Self::Hand,
            sys::ImGuiMouseCursor_NotAllowed => Self::NotAllowed,
            _ => Self::Arrow,
        }
    }
}

/// Every key the editor binds, including the modifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Minus,
    Equal,
    Plus,
    Digit0,
    A,
    C,
    D,
    F,
    G,
    N,
    O,
    R,
    S,
    V,
    X,
    Y,
    Z,
    F11,
    F12,
}

impl Key {
    pub(crate) fn code(self) -> sys::ImGuiKey {
        match self {
            Self::Tab => sys::ImGuiKey_Tab,
            Self::Left => sys::ImGuiKey_LeftArrow,
            Self::Right => sys::ImGuiKey_RightArrow,
            Self::Up => sys::ImGuiKey_UpArrow,
            Self::Down => sys::ImGuiKey_DownArrow,
            Self::PageUp => sys::ImGuiKey_PageUp,
            Self::PageDown => sys::ImGuiKey_PageDown,
            Self::Home => sys::ImGuiKey_Home,
            Self::End => sys::ImGuiKey_End,
            Self::Insert => sys::ImGuiKey_Insert,
            Self::Delete => sys::ImGuiKey_Delete,
            Self::Backspace => sys::ImGuiKey_Backspace,
            Self::Space => sys::ImGuiKey_Space,
            Self::Enter => sys::ImGuiKey_Enter,
            Self::Escape => sys::ImGuiKey_Escape,
            Self::Ctrl => sys::ImGuiMod_Ctrl,
            Self::Shift => sys::ImGuiMod_Shift,
            Self::Alt => sys::ImGuiMod_Alt,
            Self::Super => sys::ImGuiMod_Super,
            Self::Minus => sys::ImGuiKey_Minus,
            Self::Equal => sys::ImGuiKey_Equal,
            Self::Plus => sys::ImGuiKey_KeypadAdd,
            Self::Digit0 => sys::ImGuiKey_0,
            Self::A => sys::ImGuiKey_A,
            Self::C => sys::ImGuiKey_C,
            Self::D => sys::ImGuiKey_D,
            Self::F => sys::ImGuiKey_F,
            Self::G => sys::ImGuiKey_G,
            Self::N => sys::ImGuiKey_N,
            Self::O => sys::ImGuiKey_O,
            Self::R => sys::ImGuiKey_R,
            Self::S => sys::ImGuiKey_S,
            Self::V => sys::ImGuiKey_V,
            Self::X => sys::ImGuiKey_X,
            Self::Y => sys::ImGuiKey_Y,
            Self::Z => sys::ImGuiKey_Z,
            Self::F11 => sys::ImGuiKey_F11,
            Self::F12 => sys::ImGuiKey_F12,
        }
    }
}

impl Ui<'_> {
    pub fn mouse_position(&self) -> Vec2 {
        from_v(unsafe { sys::igGetMousePos() })
    }

    pub fn mouse_down(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseDown_Nil(button as i32) }
    }

    pub fn mouse_clicked(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseClicked_Bool(button as i32, false) }
    }

    pub fn mouse_released(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseReleased_Nil(button as i32) }
    }

    pub fn mouse_double_clicked(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseDoubleClicked_Nil(button as i32) }
    }

    /// True once the pointer has travelled past `threshold` with `button` held.
    pub fn mouse_dragging(&self, button: MouseButton, threshold: f32) -> bool {
        unsafe { sys::igIsMouseDragging(button as i32, threshold) }
    }

    pub fn mouse_drag_delta(&self, button: MouseButton, threshold: f32) -> Vec2 {
        from_v(unsafe { sys::igGetMouseDragDelta(button as i32, threshold) })
    }

    pub fn reset_mouse_drag_delta(&self, button: MouseButton) {
        unsafe { sys::igResetMouseDragDelta(button as i32) }
    }

    pub fn mouse_wheel(&self) -> Vec2 {
        unsafe {
            let io = sys::igGetIO_Nil();
            [(*io).MouseWheelH, (*io).MouseWheel]
        }
    }

    pub fn key_pressed(&self, key: Key) -> bool {
        unsafe { sys::igIsKeyPressed_Bool(key.code(), true) }
    }

    pub fn key_down(&self, key: Key) -> bool {
        unsafe { sys::igIsKeyDown_Nil(key.code()) }
    }

    /// True when the platform's primary modifier is held: Control, or Command on macOS.
    pub fn primary_modifier(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).KeyCtrl || (*sys::igGetIO_Nil()).KeySuper }
    }

    pub fn shift_down(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).KeyShift }
    }

    pub fn alt_down(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).KeyAlt }
    }

    pub fn item_hovered(&self) -> bool {
        unsafe { sys::igIsItemHovered(0) }
    }

    /// Hover test that keeps reporting while a pointer button is held elsewhere.
    pub fn item_hovered_allow_when_blocked(&self) -> bool {
        unsafe {
            sys::igIsItemHovered(
                sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem
                    | sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup,
            )
        }
    }

    /// Hover test that only reports once the pointer has rested, for tooltips that stand
    /// in for a `title` attribute. The delay is not shared with neighbouring items, so
    /// moving along a row of buttons waits again at each one, as a browser does.
    pub fn item_hovered_after_delay(&self) -> bool {
        unsafe {
            sys::igIsItemHovered(
                sys::ImGuiHoveredFlags_DelayNormal | sys::ImGuiHoveredFlags_NoSharedDelay,
            )
        }
    }

    pub fn item_active(&self) -> bool {
        unsafe { sys::igIsItemActive() }
    }

    pub fn item_clicked(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsItemClicked(button as i32) }
    }

    /// True on the frame a widget loses focus after its value changed.
    pub fn item_deactivated_after_edit(&self) -> bool {
        unsafe { sys::igIsItemDeactivatedAfterEdit() }
    }

    pub fn item_deactivated(&self) -> bool {
        unsafe { sys::igIsItemDeactivated() }
    }

    pub fn window_hovered(&self) -> bool {
        unsafe { sys::igIsWindowHovered(0) }
    }

    pub fn window_focused(&self) -> bool {
        unsafe { sys::igIsWindowFocused(0) }
    }

    /// True when no ImGui window sits under the pointer.
    pub fn any_window_hovered(&self) -> bool {
        unsafe { sys::igIsWindowHovered(sys::ImGuiHoveredFlags_AnyWindow) }
    }

    pub fn set_keyboard_focus_here(&mut self) {
        unsafe { sys::igSetKeyboardFocusHere(0) }
    }

    pub fn set_item_default_focus(&mut self) {
        unsafe { sys::igSetItemDefaultFocus() }
    }

    pub fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        let raw = match cursor {
            MouseCursor::Arrow => sys::ImGuiMouseCursor_Arrow,
            MouseCursor::TextInput => sys::ImGuiMouseCursor_TextInput,
            MouseCursor::ResizeAll => sys::ImGuiMouseCursor_ResizeAll,
            MouseCursor::ResizeNorthSouth => sys::ImGuiMouseCursor_ResizeNS,
            MouseCursor::ResizeEastWest => sys::ImGuiMouseCursor_ResizeEW,
            MouseCursor::ResizeNorthEastSouthWest => sys::ImGuiMouseCursor_ResizeNESW,
            MouseCursor::ResizeNorthWestSouthEast => sys::ImGuiMouseCursor_ResizeNWSE,
            MouseCursor::Hand => sys::ImGuiMouseCursor_Hand,
            MouseCursor::NotAllowed => sys::ImGuiMouseCursor_NotAllowed,
        };
        unsafe { sys::igSetMouseCursor(raw) }
    }
}
