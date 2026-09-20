//! Windows, layout, measurement and the widget set the editor draws with.
use crate::{Color, Guard, Ui, Vec2, c, from_v, v};
use bite_imgui_sys as sys;

/// How a window behaves; the editor's panels are fixed, so most chrome is switched off.
#[derive(Clone, Copy, Default)]
pub struct WindowFlags {
    pub no_title_bar: bool,
    pub no_resize: bool,
    pub no_move: bool,
    pub no_scrollbar: bool,
    pub no_collapse: bool,
    pub no_background: bool,
    pub no_bring_to_front: bool,
    pub no_saved_settings: bool,
    pub no_nav: bool,
    pub always_auto_resize: bool,
    pub horizontal_scrollbar: bool,
    pub no_scroll_with_mouse: bool,
}

impl WindowFlags {
    /// A fixed panel: no chrome, no movement and no persisted state.
    pub fn panel() -> Self {
        Self {
            no_title_bar: true,
            no_resize: true,
            no_move: true,
            no_collapse: true,
            no_bring_to_front: true,
            no_saved_settings: true,
            ..Self::default()
        }
    }

    fn raw(self) -> sys::ImGuiWindowFlags {
        let mut flags = 0;
        let mut set = |on: bool, flag: sys::ImGuiWindowFlags_| {
            if on {
                flags |= flag;
            }
        };
        set(self.no_title_bar, sys::ImGuiWindowFlags_NoTitleBar);
        set(self.no_resize, sys::ImGuiWindowFlags_NoResize);
        set(self.no_move, sys::ImGuiWindowFlags_NoMove);
        set(self.no_scrollbar, sys::ImGuiWindowFlags_NoScrollbar);
        set(self.no_collapse, sys::ImGuiWindowFlags_NoCollapse);
        set(self.no_background, sys::ImGuiWindowFlags_NoBackground);
        set(
            self.no_bring_to_front,
            sys::ImGuiWindowFlags_NoBringToFrontOnFocus,
        );
        set(self.no_saved_settings, sys::ImGuiWindowFlags_NoSavedSettings);
        set(self.no_nav, sys::ImGuiWindowFlags_NoNav);
        set(self.always_auto_resize, sys::ImGuiWindowFlags_AlwaysAutoResize);
        set(
            self.horizontal_scrollbar,
            sys::ImGuiWindowFlags_HorizontalScrollbar,
        );
        set(
            self.no_scroll_with_mouse,
            sys::ImGuiWindowFlags_NoScrollWithMouse,
        );
        flags
    }
}

/// Text field behavior.
#[derive(Clone, Copy, Default)]
pub struct InputFlags {
    pub read_only: bool,
    pub password: bool,
    pub chars_no_blank: bool,
    pub auto_select_all: bool,
    pub enter_returns_true: bool,
}

impl InputFlags {
    fn raw(self) -> sys::ImGuiInputTextFlags {
        let mut flags = 0;
        let mut set = |on: bool, flag: sys::ImGuiInputTextFlags_| {
            if on {
                flags |= flag;
            }
        };
        set(self.read_only, sys::ImGuiInputTextFlags_ReadOnly);
        set(self.password, sys::ImGuiInputTextFlags_Password);
        set(self.chars_no_blank, sys::ImGuiInputTextFlags_CharsNoBlank);
        set(self.auto_select_all, sys::ImGuiInputTextFlags_AutoSelectAll);
        set(
            self.enter_returns_true,
            sys::ImGuiInputTextFlags_EnterReturnsTrue,
        );
        flags
    }
}

impl Ui<'_> {
    // -- Windows and containers ------------------------------------------------------

    pub fn set_next_window_position(&mut self, position: Vec2) {
        unsafe { sys::igSetNextWindowPos(v(position), sys::ImGuiCond_Always, v([0.0, 0.0])) }
    }

    /// Focuses the next window, so a field inside it can take the keyboard.
    pub fn set_next_window_focus(&mut self) {
        unsafe { sys::igSetNextWindowFocus() }
    }

    pub fn set_next_window_size(&mut self, size: Vec2) {
        unsafe { sys::igSetNextWindowSize(v(size), sys::ImGuiCond_Always) }
    }

    pub fn set_next_window_size_once(&mut self, size: Vec2) {
        unsafe { sys::igSetNextWindowSize(v(size), sys::ImGuiCond_FirstUseEver) }
    }

    /// Centers the next window on the viewport, as every modal does.
    pub fn center_next_window(&mut self) {
        unsafe {
            let viewport = sys::igGetMainViewport();
            let center = sys::ImVec2 {
                x: (*viewport).Pos.x + (*viewport).Size.x * 0.5,
                y: (*viewport).Pos.y + (*viewport).Size.y * 0.5,
            };
            sys::igSetNextWindowPos(center, sys::ImGuiCond_Always, v([0.5, 0.5]));
        }
    }

    pub fn window(&mut self, title: &str, body: impl FnOnce(&mut Self)) {
        self.window_with(title, WindowFlags::default(), body);
    }

    /// A window with a close button in its title bar, which clears `open` when pressed.
    pub fn window_closable(
        &mut self,
        title: &str,
        flags: WindowFlags,
        open: &mut bool,
        body: impl FnOnce(&mut Self),
    ) {
        let visible = unsafe { sys::igBegin(c(title).as_ptr(), open, flags.raw()) };
        let _guard = Guard(|| unsafe { sys::igEnd() });
        if visible {
            body(self);
        }
    }

    pub fn window_with(
        &mut self,
        title: &str,
        flags: WindowFlags,
        body: impl FnOnce(&mut Self),
    ) {
        let visible = unsafe { sys::igBegin(c(title).as_ptr(), std::ptr::null_mut(), flags.raw()) };
        let _guard = Guard(|| unsafe { sys::igEnd() });
        if visible {
            body(self);
        }
    }

    /// A scrollable sub-region. `size` components of zero fill the remaining space.
    pub fn child(&mut self, id: &str, size: Vec2, border: bool, body: impl FnOnce(&mut Self)) {
        self.child_with(id, size, border, WindowFlags::default(), body);
    }

    pub fn child_with(
        &mut self,
        id: &str,
        size: Vec2,
        border: bool,
        flags: WindowFlags,
        body: impl FnOnce(&mut Self),
    ) {
        let child_flags = if border {
            sys::ImGuiChildFlags_Border
        } else {
            0
        };
        let visible =
            unsafe { sys::igBeginChild_Str(c(id).as_ptr(), v(size), child_flags, flags.raw()) };
        let _guard = Guard(|| unsafe { sys::igEndChild() });
        if visible {
            body(self);
        }
    }

    // -- Menus -----------------------------------------------------------------------

    pub fn main_menu_bar(&mut self, body: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginMainMenuBar() } {
            body(self);
            unsafe { sys::igEndMainMenuBar() }
        }
    }

    pub fn menu(&mut self, label: &str, body: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginMenu(c(label).as_ptr(), true) } {
            body(self);
            unsafe { sys::igEndMenu() }
        }
    }

    pub fn menu_item(&mut self, label: &str, shortcut: &str, selected: bool, enabled: bool) -> bool {
        let label = c(label);
        // The shortcut string has to outlive the call. Building it inside the argument would
        // drop it at the end of the block that made it, leaving Dear ImGui a dangling pointer
        // and the accelerator missing from the row.
        let shortcut = (!shortcut.is_empty()).then(|| c(shortcut));
        let shortcut = shortcut
            .as_ref()
            .map_or(std::ptr::null(), |text| text.as_ptr());
        unsafe { sys::igMenuItem_Bool(label.as_ptr(), shortcut, selected, enabled) }
    }

    pub fn separator(&mut self) {
        unsafe { sys::igSeparator() }
    }

    // -- Popups and modals -----------------------------------------------------------

    pub fn open_popup(&mut self, id: &str) {
        unsafe { sys::igOpenPopup_Str(c(id).as_ptr(), 0) }
    }

    pub fn popup(&mut self, id: &str, body: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginPopup(c(id).as_ptr(), 0) } {
            body(self);
            unsafe { sys::igEndPopup() }
        }
    }

    /// A modal popup. `open` is cleared when the popup closes itself.
    pub fn modal(&mut self, id: &str, open: &mut bool, flags: WindowFlags, body: impl FnOnce(&mut Self)) {
        let mut raw_open = *open;
        if unsafe { sys::igBeginPopupModal(c(id).as_ptr(), &mut raw_open, flags.raw()) } {
            body(self);
            unsafe { sys::igEndPopup() }
        }
        *open = raw_open;
    }

    /// A modal popup without a close control, matching dialogs that offer explicit buttons.
    pub fn modal_without_close(&mut self, id: &str, flags: WindowFlags, body: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginPopupModal(c(id).as_ptr(), std::ptr::null_mut(), flags.raw()) } {
            body(self);
            unsafe { sys::igEndPopup() }
        }
    }

    pub fn close_current_popup(&mut self) {
        unsafe { sys::igCloseCurrentPopup() }
    }

    pub fn popup_open(&self, id: &str) -> bool {
        unsafe { sys::igIsPopupOpen_Str(c(id).as_ptr(), 0) }
    }

    // -- Tooltips --------------------------------------------------------------------

    pub fn tooltip(&mut self, body: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginTooltip() } {
            body(self);
            unsafe { sys::igEndTooltip() }
        }
    }

    // -- Layout and measurement ------------------------------------------------------

    pub fn content_region_available(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetContentRegionAvail(&mut out) };
        from_v(out)
    }

    pub fn cursor_screen_position(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetCursorScreenPos(&mut out) };
        from_v(out)
    }

    pub fn set_cursor_screen_position(&mut self, position: Vec2) {
        unsafe { sys::igSetCursorScreenPos(v(position)) }
    }

    pub fn cursor_position(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetCursorPos(&mut out) };
        from_v(out)
    }

    pub fn set_cursor_position(&mut self, position: Vec2) {
        unsafe { sys::igSetCursorPos(v(position)) }
    }

    pub fn window_position(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetWindowPos(&mut out) };
        from_v(out)
    }

    pub fn window_size(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetWindowSize(&mut out) };
        from_v(out)
    }

    pub fn item_rect_min(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetItemRectMin(&mut out) };
        from_v(out)
    }

    pub fn item_rect_max(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetItemRectMax(&mut out) };
        from_v(out)
    }

    pub fn item_rect_size(&self) -> Vec2 {
        let mut out = sys::ImVec2::default();
        unsafe { sys::igGetItemRectSize(&mut out) };
        from_v(out)
    }

    pub fn calc_text_size(&self, text: &str) -> Vec2 {
        let text = c(text);
        let mut out = sys::ImVec2::default();
        unsafe { sys::igCalcTextSize(&mut out, text.as_ptr(), std::ptr::null(), false, -1.0) };
        from_v(out)
    }

    pub fn text_line_height(&self) -> f32 {
        unsafe { sys::igGetTextLineHeight() }
    }

    pub fn frame_height(&self) -> f32 {
        unsafe { sys::igGetFrameHeight() }
    }

    pub fn same_line(&mut self) {
        unsafe { sys::igSameLine(0.0, -1.0) }
    }

    pub fn same_line_at(&mut self, offset: f32, spacing: f32) {
        unsafe { sys::igSameLine(offset, spacing) }
    }

    pub fn spacing(&mut self) {
        unsafe { sys::igSpacing() }
    }

    pub fn new_line(&mut self) {
        unsafe { sys::igNewLine() }
    }

    pub fn dummy(&mut self, size: Vec2) {
        unsafe { sys::igDummy(v(size)) }
    }

    pub fn indent(&mut self, amount: f32) {
        unsafe { sys::igIndent(amount) }
    }

    pub fn unindent(&mut self, amount: f32) {
        unsafe { sys::igUnindent(amount) }
    }

    pub fn group<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        unsafe { sys::igBeginGroup() };
        let result = body(self);
        unsafe { sys::igEndGroup() };
        result
    }

    pub fn set_next_item_width(&mut self, width: f32) {
        unsafe { sys::igSetNextItemWidth(width) }
    }

    pub fn push_id(&mut self, id: &str) -> IdGuard {
        unsafe { sys::igPushID_Str(c(id).as_ptr()) };
        IdGuard
    }

    pub fn scroll_y(&self) -> f32 {
        unsafe { sys::igGetScrollY() }
    }

    pub fn set_scroll_y(&mut self, value: f32) {
        unsafe { sys::igSetScrollY_Float(value) }
    }

    /// How far the window can scroll down, for a view that follows its newest line.
    pub fn scroll_max_y(&self) -> f32 {
        unsafe { sys::igGetScrollMaxY() }
    }

    pub fn scroll_x(&self) -> f32 {
        unsafe { sys::igGetScrollX() }
    }

    pub fn set_scroll_x(&mut self, value: f32) {
        unsafe { sys::igSetScrollX_Float(value) }
    }

    pub fn scroll_max_x(&self) -> f32 {
        unsafe { sys::igGetScrollMaxX() }
    }

    pub fn scroll_into_view(&mut self) {
        unsafe { sys::igSetScrollHereY(0.5) }
    }

    // -- Text ------------------------------------------------------------------------

    pub fn text(&mut self, text: &str) {
        unsafe { sys::igTextUnformatted(c(text).as_ptr(), std::ptr::null()) }
    }

    pub fn text_colored(&mut self, color: Color, text: &str) {
        self.with_colors(&[(crate::StyleColor::Text, color)], |ui| ui.text(text));
    }

    pub fn text_wrapped(&mut self, text: &str) {
        unsafe {
            sys::igPushTextWrapPos(0.0);
            sys::igTextUnformatted(c(text).as_ptr(), std::ptr::null());
            sys::igPopTextWrapPos();
        }
    }

    /// Wraps at `width` from the current cursor.
    ///
    /// A plain `text_wrapped` inside an auto-sized window wraps at the window's own edge,
    /// which is itself derived from the content, so the text collapses to one glyph a line.
    /// Naming the width breaks that circle.
    pub fn text_wrapped_at(&mut self, text: &str, width: f32) {
        // The wrap position is window-local, not a screen coordinate. Passing a screen x
        // puts it far beyond the window and nothing ever wraps.
        let wrap = self.cursor_position()[0] + width;
        unsafe {
            sys::igPushTextWrapPos(wrap);
            sys::igTextUnformatted(c(text).as_ptr(), std::ptr::null());
            sys::igPopTextWrapPos();
        }
    }

    /// Draws text clipped to `width`, appending an ellipsis when it does not fit.
    pub fn text_ellipsized(&mut self, text: &str, width: f32) {
        let full = self.calc_text_size(text);
        if full[0] <= width || text.is_empty() {
            return self.text(text);
        }
        let ellipsis = "...";
        let reserve = self.calc_text_size(ellipsis)[0];
        let mut end = text.len();
        while end > 0 {
            if !text.is_char_boundary(end) {
                end -= 1;
                continue;
            }
            if self.calc_text_size(&text[..end])[0] + reserve <= width {
                break;
            }
            end -= 1;
        }
        let truncated = format!("{}{ellipsis}", &text[..end]);
        self.text(&truncated);
    }

    // -- Buttons and selection -------------------------------------------------------

    pub fn button(&mut self, label: &str) -> bool {
        unsafe { sys::igButton(c(label).as_ptr(), v([0.0, 0.0])) }
    }

    pub fn button_sized(&mut self, label: &str, size: Vec2) -> bool {
        unsafe { sys::igButton(c(label).as_ptr(), v(size)) }
    }

    /// A hit region with no visuals of its own, for hand-drawn controls.
    pub fn invisible_button(&mut self, id: &str, size: Vec2) -> bool {
        unsafe { sys::igInvisibleButton(c(id).as_ptr(), v(size), 0) }
    }

    pub fn selectable(&mut self, label: &str, selected: bool) -> bool {
        unsafe { sys::igSelectable_Bool(c(label).as_ptr(), selected, 0, v([0.0, 0.0])) }
    }

    pub fn selectable_sized(&mut self, label: &str, selected: bool, size: Vec2) -> bool {
        unsafe { sys::igSelectable_Bool(c(label).as_ptr(), selected, 0, v(size)) }
    }

    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> bool {
        unsafe { sys::igCheckbox(c(label).as_ptr(), value) }
    }

    pub fn collapsing_header(&mut self, label: &str, default_open: bool) -> bool {
        let flags = if default_open {
            sys::ImGuiTreeNodeFlags_DefaultOpen
        } else {
            0
        };
        unsafe { sys::igCollapsingHeader_TreeNodeFlags(c(label).as_ptr(), flags) }
    }

    pub fn progress_bar(&mut self, fraction: f32, size: Vec2, overlay: Option<&str>) {
        let overlay = overlay.map(c);
        unsafe {
            sys::igProgressBar(
                fraction.clamp(0.0, 1.0),
                v(size),
                overlay
                    .as_ref()
                    .map_or(std::ptr::null(), |text| text.as_ptr()),
            )
        }
    }

    // -- Numbers ---------------------------------------------------------------------

    pub fn drag_float(&mut self, label: &str, value: &mut f32, speed: f32, min: f32, max: f32) -> bool {
        unsafe {
            sys::igDragFloat(
                c(label).as_ptr(),
                value,
                speed,
                min,
                max,
                c("%.3f").as_ptr(),
                0,
            )
        }
    }

    pub fn slider_float(&mut self, label: &str, value: &mut f32, min: f32, max: f32) -> bool {
        unsafe {
            sys::igSliderFloat(c(label).as_ptr(), value, min, max, c("%.3f").as_ptr(), 0)
        }
    }

    pub fn slider_int(&mut self, label: &str, value: &mut i32, min: i32, max: i32) -> bool {
        unsafe { sys::igSliderInt(c(label).as_ptr(), value, min, max, c("%d").as_ptr(), 0) }
    }

    pub fn drag_float_n(&mut self, label: &str, values: &mut [f32], speed: f32) -> bool {
        if values.is_empty() || values.len() > 4 {
            return false;
        }
        unsafe {
            sys::igDragScalarN(
                c(label).as_ptr(),
                sys::ImGuiDataType_Float,
                values.as_mut_ptr().cast(),
                values.len() as i32,
                speed,
                std::ptr::null(),
                std::ptr::null(),
                c("%.3f").as_ptr(),
                0,
            )
        }
    }

    /// A numeric text field, matching the Electron `input type="number"` controls.
    pub fn input_float(&mut self, label: &str, value: &mut f32, step: f32) -> bool {
        unsafe {
            sys::igInputFloat(
                c(label).as_ptr(),
                value,
                step,
                0.0,
                c("%.3f").as_ptr(),
                0,
            )
        }
    }

    pub fn input_int(&mut self, label: &str, value: &mut i32, step: i32) -> bool {
        unsafe { sys::igInputInt(c(label).as_ptr(), value, step, 0, 0) }
    }

    pub fn color_edit4(&mut self, label: &str, values: &mut [f32; 4]) -> bool {
        unsafe {
            sys::igColorEdit4(
                c(label).as_ptr(),
                values.as_mut_ptr(),
                // Without this the editor packs four drag fields into the row, which is
                // unreadable at an inspector's width. A swatch opens the picker instead.
                sys::ImGuiColorEditFlags_AlphaBar
                    | sys::ImGuiColorEditFlags_AlphaPreviewHalf
                    | sys::ImGuiColorEditFlags_NoInputs,
            )
        }
    }

    /// The full picker, for use inside a popup opened from a swatch.
    pub fn color_picker4(&mut self, label: &str, values: &mut [f32; 4]) -> bool {
        unsafe {
            sys::igColorPicker4(
                c(label).as_ptr(),
                values.as_mut_ptr(),
                sys::ImGuiColorEditFlags_AlphaBar,
                std::ptr::null(),
            )
        }
    }

    pub fn color_button(&mut self, id: &str, color: Color, size: Vec2) -> bool {
        unsafe {
            sys::igColorButton(
                c(id).as_ptr(),
                color.raw(),
                sys::ImGuiColorEditFlags_AlphaPreviewHalf,
                v(size),
            )
        }
    }

    // -- Text entry ------------------------------------------------------------------

    pub fn input_text(&mut self, label: &str, text: &mut String) -> bool {
        self.input_text_with(label, text, "", InputFlags::default())
    }

    pub fn input_text_with(
        &mut self,
        label: &str,
        text: &mut String,
        hint: &str,
        flags: InputFlags,
    ) -> bool {
        let mut buffer = vec![0u8; text.len().max(1024) + 256];
        buffer[..text.len()].copy_from_slice(text.as_bytes());
        let changed = unsafe {
            if hint.is_empty() {
                sys::igInputText(
                    c(label).as_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    flags.raw(),
                    None,
                    std::ptr::null_mut(),
                )
            } else {
                sys::igInputTextWithHint(
                    c(label).as_ptr(),
                    c(hint).as_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    flags.raw(),
                    None,
                    std::ptr::null_mut(),
                )
            }
        };
        if changed {
            *text = Self::buffer_string(&buffer);
        }
        changed
    }

    pub fn input_text_multiline(
        &mut self,
        label: &str,
        text: &mut String,
        size: Vec2,
        flags: InputFlags,
    ) -> bool {
        let mut buffer = vec![0u8; text.len().max(4096) + 1024];
        buffer[..text.len()].copy_from_slice(text.as_bytes());
        let changed = unsafe {
            sys::igInputTextMultiline(
                c(label).as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                v(size),
                flags.raw(),
                None,
                std::ptr::null_mut(),
            )
        };
        if changed {
            *text = Self::buffer_string(&buffer);
        }
        changed
    }

    // -- Combos ----------------------------------------------------------------------

    /// A drop-down list. Returns true when the selection changed.
    /// Bounds the next window's size. A combo uses this in place of its own cap, which is
    /// eight rows of Dear ImGui's row height rather than eight of the caller's.
    pub fn set_next_window_size_constraints(&mut self, min: Vec2, max: Vec2) {
        unsafe {
            sys::igSetNextWindowSizeConstraints(
                v(min),
                v(max),
                None,
                std::ptr::null_mut(),
            )
        }
    }

    /// Opens a combo's list. The caller draws the rows and calls [`Ui::end_combo`], which
    /// lets it style the list on its own terms rather than the closed control's.
    pub fn begin_combo(&mut self, label: &str, preview: &str) -> bool {
        let label = c(label);
        let preview = c(preview);
        unsafe { sys::igBeginCombo(label.as_ptr(), preview.as_ptr(), 0) }
    }

    pub fn end_combo(&mut self) {
        unsafe { sys::igEndCombo() }
    }

    pub fn combo(&mut self, label: &str, current: &mut usize, items: &[String]) -> bool {
        if items.is_empty() {
            return false;
        }
        let index = (*current).min(items.len() - 1);
        let preview = c(&items[index]);
        let mut changed = false;
        if unsafe { sys::igBeginCombo(c(label).as_ptr(), preview.as_ptr(), 0) } {
            for (position, item) in items.iter().enumerate() {
                let selected = position == index;
                if unsafe {
                    sys::igSelectable_Bool(c(item).as_ptr(), selected, 0, v([0.0, 0.0]))
                } {
                    *current = position;
                    changed = true;
                }
                if selected {
                    unsafe { sys::igSetItemDefaultFocus() };
                }
            }
            unsafe { sys::igEndCombo() };
        }
        changed
    }

    // -- Images ----------------------------------------------------------------------

    pub fn image(&mut self, texture: u64, size: Vec2) {
        unsafe {
            sys::igImage(
                texture as sys::ImTextureID,
                v(size),
                v([0.0, 0.0]),
                v([1.0, 1.0]),
                sys::ImVec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
                sys::ImVec4::default(),
            )
        }
    }

    pub fn image_button(&mut self, id: &str, texture: u64, size: Vec2) -> bool {
        unsafe {
            sys::igImageButton(
                c(id).as_ptr(),
                texture as sys::ImTextureID,
                v(size),
                v([0.0, 0.0]),
                v([1.0, 1.0]),
                sys::ImVec4::default(),
                sys::ImVec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
            )
        }
    }

    // -- Drag and drop ---------------------------------------------------------------

    /// Marks the last item as a drag source carrying `payload`.
    pub fn drag_source(&mut self, kind: &str, payload: &str, preview: impl FnOnce(&mut Self)) {
        if unsafe { sys::igBeginDragDropSource(0) } {
            let bytes = payload.as_bytes();
            unsafe {
                sys::igSetDragDropPayload(
                    c(kind).as_ptr(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    sys::ImGuiCond_Once,
                );
            }
            preview(self);
            unsafe { sys::igEndDragDropSource() };
        }
    }

    /// Accepts a payload of `kind` dropped anywhere inside `min`..`max`.
    ///
    /// The plain target binds to the last item; a panel that draws itself has no such item,
    /// so the rectangle is named directly.
    pub fn drag_target_rect(&mut self, kind: &str, min: Vec2, max: Vec2, id: &str) -> Option<String> {
        let bb = sys::ImRect {
            Min: v(min),
            Max: v(max),
        };
        let id = unsafe { sys::igGetID_Str(c(id).as_ptr()) };
        let mut result = None;
        if unsafe { sys::igBeginDragDropTargetCustom(bb, id) } {
            result = self.accept_payload(kind);
            unsafe { sys::igEndDragDropTarget() };
        }
        result
    }

    fn accept_payload(&mut self, kind: &str) -> Option<String> {
        let payload = unsafe { sys::igAcceptDragDropPayload(c(kind).as_ptr(), 0) };
        if payload.is_null() {
            return None;
        }
        unsafe {
            let length = (*payload).DataSize.max(0) as usize;
            let bytes = std::slice::from_raw_parts((*payload).Data as *const u8, length);
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
    }

    /// Accepts a payload of `kind` dropped on the last item.
    pub fn drag_target(&mut self, kind: &str) -> Option<String> {
        let mut result = None;
        if unsafe { sys::igBeginDragDropTarget() } {
            let payload = unsafe { sys::igAcceptDragDropPayload(c(kind).as_ptr(), 0) };
            if !payload.is_null() {
                unsafe {
                    let length = (*payload).DataSize.max(0) as usize;
                    let bytes =
                        std::slice::from_raw_parts((*payload).Data as *const u8, length);
                    result = Some(String::from_utf8_lossy(bytes).into_owned());
                }
            }
            unsafe { sys::igEndDragDropTarget() };
        }
        result
    }
}

/// Restores the identifier stack when dropped.
pub struct IdGuard;

impl Drop for IdGuard {
    fn drop(&mut self) {
        unsafe { sys::igPopID() }
    }
}
