//! The node creation menu, from `src/renderer/nodeEditor/NodeContextMenu.svelte`.
//!
//! It opens from a right click, from Space or Tab, and from a wire dropped on empty canvas.
//! With no search text it browses by category; once text is typed it shows one flat list.
use crate::{
    canvas::state::{PendingWire, WireEnd},
    controls,
    panels::library::{self, Entry},
    theme,
};
use bite_core::{Registry, graph::WireType};
use bite_imgui::{Key, MouseCursor, Rounding, StyleColor, StyleVar, Ui, Vec2};
use bite_schema::{ParamType, PortType};

/// What the menu produced.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Create this entry at the recorded position, connecting it if a wire was dropped.
    Create(Entry),
    GroupSelection,
    UngroupSelection,
    Dismissed,
}

/// The menu's own state while it is open.
pub struct CreateMenu {
    pub open: bool,
    pub query: String,
    /// Where the new node goes, in graph coordinates.
    pub position: Vec2,
    pub pending: Option<PendingWire>,
    /// Set on the frame the menu opens so the search field can take focus once.
    focus_search: bool,
    active_index: usize,
    pub can_group: bool,
    pub can_ungroup: bool,
    tooltip: controls::HoverTimer,
    /// The category whose flyout is showing. It outlives the hover on the row itself, so
    /// the pointer can travel into the flyout without it closing on the way.
    open_category: Option<String>,
    /// Last frame's flyout rectangle, which counts as part of its category for hovering.
    sub_rect: Option<(Vec2, Vec2)>,
    /// Set when the keyboard, rather than the pointer, chose the open category.
    focus_search_pending: bool,
}

impl Default for CreateMenu {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            position: [0.0, 0.0],
            pending: None,
            focus_search: false,
            active_index: 0,
            can_group: false,
            can_ungroup: false,
            tooltip: controls::HoverTimer::default(),
            open_category: None,
            sub_rect: None,
            focus_search_pending: false,
        }
    }
}

impl CreateMenu {
    /// The category whose flyout is open, if any.
    pub fn open_category(&self) -> Option<&str> {
        self.open_category.as_deref()
    }

    /// Opens the menu at a graph position, optionally completing a dropped wire.
    pub fn open_at(&mut self, position: Vec2, pending: Option<PendingWire>) {
        self.open = true;
        self.query.clear();
        self.position = position;
        self.pending = pending;
        self.focus_search = true;
        self.focus_search_pending = true;
        self.active_index = 0;
        self.tooltip.reset();
        self.open_category = None;
        self.sub_rect = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.pending = None;
        self.query.clear();
        self.open_category = None;
        self.sub_rect = None;
        self.focus_search_pending = false;
    }

    /// Draws the menu at `anchor`, a screen position. Returns an outcome when it closes.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        anchor: Vec2,
        viewport_size: Vec2,
        registry: &Registry,
        delta: f32,
    ) -> Option<Outcome> {
        if !self.open {
            return None;
        }
        let entries = self.candidates(registry);
        let searching = !self.query.trim().is_empty();
        let groups = library::grouped(&entries, &self.query);
        let flat: Vec<Entry> = entries
            .iter()
            .filter(|entry| library::matches(entry, &self.query))
            .cloned()
            .collect();

        // The panel flips above the pointer when there is more room there.
        let space_below = viewport_size[1] - anchor[1];
        let space_above = anchor[1];
        let flip_up = space_above > space_below;
        // The panel is only as tall as its rows, up to what the ceiling and the space on
        // screen allow; a fixed height would leave a gap below the last row.
        let rows = if searching {
            flat.len().max(1)
        } else {
            groups.len().max(1)
        };
        let content = theme::CTX_PADDING_Y * 2.0
            + theme::CTX_SEARCH_HEIGHT
            + theme::CTX_SEARCH_GAP
            + rows as f32 * theme::CTX_ROW_HEIGHT
            + self.action_height();
        let room = (if flip_up { space_above } else { space_below }) - 16.0;
        let max_height = content.min(theme::CTX_MAX_HEIGHT).min(room).max(120.0);
        let left = anchor[0].min(viewport_size[0] - theme::CTX_WIDTH - 8.0).max(0.0);
        let top = if flip_up {
            (anchor[1] - max_height).max(0.0)
        } else {
            anchor[1]
        };

        let mut outcome = None;
        // A click anywhere outside the panel dismisses the menu, as its backdrop does.
        let pointer = ui.mouse_position();
        let in_panel = pointer[0] >= left
            && pointer[0] <= left + theme::CTX_WIDTH
            && pointer[1] >= top
            && pointer[1] <= top + max_height;
        // A flyout can hang below the panel, so it is tested as its own rectangle.
        let in_flyout = self
            .sub_rect
            .is_some_and(|(min, max)| controls::point_in(pointer, min, max));
        if !in_panel && !in_flyout && ui.mouse_clicked(bite_imgui::MouseButton::Left) {
            self.close();
            return Some(Outcome::Dismissed);
        }

        ui.set_next_window_position([left, top]);
        ui.set_next_window_size([theme::CTX_WIDTH, max_height]);
        if self.focus_search_pending {
            // The search field can only take the keyboard once its window holds focus.
            ui.set_next_window_focus();
            self.focus_search_pending = false;
        }
        // The menu must draw above the panels, so it keeps the default stacking. Keyboard
        // navigation stays on, because the search field is focused through it.
        let flags = bite_imgui::WindowFlags {
            no_title_bar: true,
            no_resize: true,
            no_move: true,
            no_collapse: true,
            no_saved_settings: true,
            no_scrollbar: true,
            ..bite_imgui::WindowFlags::default()
        };

        ui.with_style(
            &[
                StyleVar::WindowPadding([0.0, 4.0]),
                StyleVar::WindowRounding(theme::CTX_RADIUS),
                StyleVar::WindowBorderSize(1.0),
                StyleVar::ItemSpacing([0.0, 0.0]),
            ],
            |ui| {
                ui.with_colors(
                    &[
                        (StyleColor::WindowBg, theme::CTX_BG),
                        (StyleColor::Border, theme::CTX_BORDER),
                        (StyleColor::Text, theme::CTX_TEXT),
                    ],
                    |ui| {
                        ui.window_with("##create-node", flags, |ui| {
                            outcome = self.body(ui, &groups, &flat, searching, delta);
                        });
                    },
                )
            },
        );

        // Escape closes the menu, and a click outside dismisses it.
        if ui.key_pressed(Key::Escape) {
            self.close();
            return Some(Outcome::Dismissed);
        }
        if let Some(outcome) = &outcome {
            if !matches!(outcome, Outcome::Dismissed) {
                self.close();
            }
        }
        outcome
    }

    fn body(
        &mut self,
        ui: &mut Ui,
        groups: &[(String, Vec<Entry>)],
        flat: &[Entry],
        searching: bool,
        delta: f32,
    ) -> Option<Outcome> {
        let width = theme::CTX_WIDTH;
        ui.set_cursor_screen_position([
            ui.cursor_screen_position()[0] + 6.0,
            ui.cursor_screen_position()[1],
        ]);
        if self.focus_search {
            // Focus is taken once, so that typing is not interrupted on later frames.
            ui.set_keyboard_focus_here();
            self.focus_search = false;
        }
        controls::search_input(ui, "create-search", &mut self.query, "Search", width - 12.0);
        ui.dummy([width, theme::CTX_SEARCH_GAP]);

        // Arrow keys move the highlight without wrapping, as the Svelte menu does.
        let count = if searching { flat.len() } else { groups.len() };
        if count > 0 {
            let mut moved = false;
            if ui.key_pressed(Key::Down) {
                self.active_index = (self.active_index + 1).min(count - 1);
                moved = true;
            }
            if ui.key_pressed(Key::Up) {
                self.active_index = self.active_index.saturating_sub(1);
                moved = true;
            }
            // Browsing by keyboard opens the highlighted category's flyout, so the arrow
            // keys show the same thing the pointer would.
            if moved && !searching {
                self.open_category = groups
                    .get(self.active_index)
                    .map(|(category, _)| category.clone());
            }
        }
        if !searching && ui.key_pressed(Key::Right) {
            self.open_category = groups
                .get(self.active_index)
                .map(|(category, _)| category.clone());
        }
        if !searching && ui.key_pressed(Key::Left) {
            self.open_category = None;
        }

        let mut outcome = None;
        self.sub_rect = None;
        let body_height = (ui.content_region_available()[1] - self.action_height()).max(40.0);
        ui.child("create-list", [width, body_height], false, |ui| {
            if searching {
                if flat.is_empty() {
                    ui.dummy([width, 6.0]);
                    ui.set_cursor_screen_position([
                        ui.cursor_screen_position()[0] + 12.0,
                        ui.cursor_screen_position()[1],
                    ]);
                    controls::hint(ui, "No matching nodes.");
                    return;
                }
                for (index, entry) in flat.iter().enumerate() {
                    if self.result_row(ui, entry, index == self.active_index, width, delta, true) {
                        outcome = Some(Outcome::Create(entry.clone()));
                    }
                }
                if ui.key_pressed(Key::Enter) {
                    if let Some(entry) = flat.get(self.active_index) {
                        outcome = Some(Outcome::Create(entry.clone()));
                    }
                }
            } else {
                for (index, (category, entries)) in groups.iter().enumerate() {
                    if let Some(entry) =
                        self.category_row(ui, category, entries, index == self.active_index, width, delta)
                    {
                        outcome = Some(Outcome::Create(entry));
                    }
                }
            }
        });

        // The group actions sit at the bottom, each behind its own divider.
        if self.can_group || self.can_ungroup {
            let origin = ui.cursor_screen_position();
            ui.draw_list().line(
                [origin[0], origin[1]],
                [origin[0] + width, origin[1]],
                theme::CTX_SEPARATOR,
                1.0,
            );
            ui.dummy([width, 1.0]);
        }
        if self.can_group && self.action_row(ui, "Group Selection", "Ctrl+G", width) {
            outcome = Some(Outcome::GroupSelection);
        }
        if self.can_ungroup && self.action_row(ui, "Ungroup", "Ctrl+Shift+G", width) {
            outcome = Some(Outcome::UngroupSelection);
        }
        outcome
    }

    fn action_height(&self) -> f32 {
        let rows = usize::from(self.can_group) + usize::from(self.can_ungroup);
        if rows == 0 {
            0.0
        } else {
            rows as f32 * theme::CTX_ROW_HEIGHT + 2.0
        }
    }

    /// A flat search result: the label on the left and its category on the right.
    fn result_row(
        &mut self,
        ui: &mut Ui,
        entry: &Entry,
        active: bool,
        width: f32,
        delta: f32,
        show_category: bool,
    ) -> bool {
        let height = theme::CTX_ROW_HEIGHT;
        let origin = ui.cursor_screen_position();
        let clicked = ui.invisible_button(&format!("##result-{}", entry.id), [width, height]);
        let hovered = ui.item_hovered();
        if hovered {
            ui.set_mouse_cursor(MouseCursor::Hand);
        }
        let list = ui.draw_list();
        if hovered || active {
            list.rect(
                [origin[0], origin[1]],
                [origin[0] + width, origin[1] + height],
                theme::CTX_ITEM_HOVER_BG,
                0.0,
                Rounding::None,
            );
        }
        let category_size = if show_category {
            list.measure(theme::face::SMALL_MONO, &entry.category)
        } else {
            [0.0, 0.0]
        };
        controls::draw_in_row(
            ui,
            origin[0] + 12.0,
            origin[1],
            height,
            theme::CTX_TEXT,
            theme::face::BODY,
            &entry.label,
        );
        if show_category {
            controls::draw_in_row(
                ui,
                origin[0] + width - 10.0 - category_size[0],
                origin[1],
                height,
                theme::CTX_TEXT_MUTED.with_alpha(0.85),
                theme::face::SMALL_MONO,
                &entry.category,
            );
        }
        if !entry.description.is_empty()
            && self
                .tooltip
                .poll(&entry.id, hovered, delta, theme::TOOLTIP_DELAY_MENU)
        {
            library::draw_tooltip(ui, &entry.description);
        }
        clicked
    }

    /// A category row that opens a flyout of its entries.
    fn category_row(
        &mut self,
        ui: &mut Ui,
        category: &str,
        entries: &[Entry],
        active: bool,
        width: f32,
        delta: f32,
    ) -> Option<Entry> {
        let height = theme::CTX_ROW_HEIGHT;
        let origin = ui.cursor_screen_position();
        ui.invisible_button(&format!("##cat-{category}"), [width, height]);
        let hovered = ui.item_hovered();
        if hovered {
            ui.set_mouse_cursor(MouseCursor::Hand);
        }
        let list = ui.draw_list();
        if hovered || active {
            list.rect(
                [origin[0], origin[1]],
                [origin[0] + width, origin[1] + height],
                theme::CTX_ITEM_HOVER_BG,
                0.0,
                Rounding::None,
            );
        }
        list.text_with_face(
            [origin[0] + 12.0, origin[1] + 5.0],
            theme::CTX_TEXT,
            theme::face::BODY,
            category,
        );
        list.text_with_face(
            [origin[0] + width - 18.0, origin[1] + 5.0],
            theme::CTX_TEXT_MUTED,
            theme::face::BODY,
            ">",
        );

        // Pointing at a row opens its flyout; the flyout then stays open until another
        // row is pointed at, so the pointer can cross the gap between them.
        if hovered {
            self.open_category = Some(category.to_string());
        }
        if self.open_category.as_deref() != Some(category) {
            return None;
        }
        let mut chosen = None;
        let sub_left = origin[0] + width + 4.0;
        let sub_height = (entries.len() as f32 * theme::CTX_ROW_HEIGHT
            + theme::CTX_PADDING_Y * 2.0)
            .min(theme::CTX_MAX_HEIGHT);
        self.sub_rect = Some((
            [sub_left, origin[1]],
            [sub_left + theme::CTX_SUB_WIDTH, origin[1] + sub_height],
        ));
        ui.set_next_window_position([sub_left, origin[1]]);
        ui.set_next_window_size([theme::CTX_SUB_WIDTH, sub_height]);
        let flags = bite_imgui::WindowFlags {
            no_title_bar: true,
            no_resize: true,
            no_move: true,
            no_collapse: true,
            no_saved_settings: true,
            no_nav: true,
            ..bite_imgui::WindowFlags::default()
        };
        ui.with_style(
            &[
                StyleVar::WindowPadding([0.0, 4.0]),
                StyleVar::WindowRounding(theme::CTX_RADIUS),
                StyleVar::WindowBorderSize(1.0),
                StyleVar::ItemSpacing([0.0, 0.0]),
            ],
            |ui| {
                ui.with_colors(
                    &[
                        (StyleColor::WindowBg, theme::CTX_BG),
                        (StyleColor::Border, theme::CTX_BORDER),
                    ],
                    |ui| {
                        ui.window_with(&format!("##sub-{category}"), flags, |ui| {
                            for entry in entries {
                                // The flyout lists one category, so its name is not repeated.
                                if self.result_row(ui, entry, false, theme::CTX_SUB_WIDTH, delta, false) {
                                    chosen = Some(entry.clone());
                                }
                            }
                        });
                    },
                )
            },
        );
        chosen
    }

    fn action_row(&mut self, ui: &mut Ui, label: &str, shortcut: &str, width: f32) -> bool {
        let height = theme::CTX_ROW_HEIGHT;
        let origin = ui.cursor_screen_position();
        let clicked = ui.invisible_button(&format!("##action-{label}"), [width, height]);
        let hovered = ui.item_hovered();
        if hovered {
            ui.set_mouse_cursor(MouseCursor::Hand);
        }
        let list = ui.draw_list();
        if hovered {
            list.rect(
                origin,
                [origin[0] + width, origin[1] + height],
                theme::CTX_ITEM_HOVER_BG,
                0.0,
                Rounding::None,
            );
        }
        list.text_with_face(
            [origin[0] + 12.0, origin[1] + 5.0],
            if hovered {
                theme::TEXT_BRIGHT
            } else {
                theme::CTX_TEXT
            },
            theme::face::BODY,
            label,
        );
        let size = list.measure(theme::face::SMALL_MONO, shortcut);
        list.text_with_face(
            [origin[0] + width - 10.0 - size[0], origin[1] + 6.0],
            theme::CTX_TEXT_MUTED,
            theme::face::SMALL_MONO,
            shortcut,
        );
        clicked
    }

    /// The entries the menu offers, narrowed to compatible nodes when a wire was dropped.
    fn candidates(&self, registry: &Registry) -> Vec<Entry> {
        let mut entries = library::workflow_entries();
        entries.extend(library::definition_entries(registry));
        let Some(pending) = &self.pending else {
            // Browsing offers a comment as well, which a dropped wire cannot connect to.
            entries.push(comment_entry());
            return entries;
        };
        entries
            .into_iter()
            .filter(|entry| accepts_wire(entry, pending, registry))
            .collect()
    }
}

/// The comment entry, which only appears when no wire is pending.
pub fn comment_entry() -> Entry {
    Entry {
        id: "commentNode".into(),
        label: "Comment".into(),
        category: "Workflow".into(),
        description: "A note pinned to the canvas".into(),
        aliases: Vec::new(),
    }
}

/// Whether a candidate can accept the wire being dropped, matching `available` in the
/// Svelte menu plus the parameter type aliases it lists.
pub fn accepts_wire(entry: &Entry, pending: &PendingWire, registry: &Registry) -> bool {
    // A value wire is compatible with everything.
    if pending.wire == WireType::Value {
        return true;
    }
    // The built-in workflow nodes only take image wires.
    if entry.category == "Workflow" {
        if pending.wire != WireType::Image {
            return false;
        }
        // A wire leaving a source cannot terminate on an Input, which has no inputs.
        return !(entry.id == "inputNode" && pending.end == WireEnd::Source);
    }
    let Some(definition) = registry.nodes.get(&entry.id) else {
        return false;
    };
    let definition = &definition.definition;
    let port_matches = |kind: &PortType| port_wire(kind) == pending.wire;
    if definition.inputs.iter().any(|port| port_matches(&port.kind))
        || definition.outputs.iter().any(|port| port_matches(&port.kind))
    {
        return true;
    }
    definition.params.iter().any(|param| {
        if param.kind == ParamType::Enum {
            return false;
        }
        param_wire(&param.kind).is_some_and(|wire| {
            wire == WireType::Value || bite_core::graph::compatible(wire, pending.wire)
        })
    })
}

fn port_wire(kind: &PortType) -> WireType {
    match kind {
        PortType::Image => WireType::Image,
        PortType::Mask => WireType::Mask,
        PortType::Number => WireType::Number,
        PortType::Path => WireType::Path,
    }
}

fn param_wire(kind: &ParamType) -> Option<WireType> {
    Some(match kind {
        ParamType::Int | ParamType::Float => WireType::Number,
        ParamType::String => WireType::String,
        ParamType::Bool => WireType::Bool,
        ParamType::Numeric => WireType::Numeric,
        ParamType::Vector2 => WireType::Vector2,
        ParamType::Vector3 => WireType::Vector3,
        ParamType::Vector4 => WireType::Vector4,
        ParamType::Color => WireType::Color,
        ParamType::Value => WireType::Value,
        _ => return None,
    })
}

/// The first handle on a newly created node that the dropped wire can attach to.
///
/// `side` is `in` when the new node receives the wire and `out` when it supplies it.
pub fn first_matching_handle(
    entry: &Entry,
    wire: WireType,
    side: WireEnd,
    registry: &Registry,
) -> Option<String> {
    let incoming = side == WireEnd::Target;
    if entry.category == "Workflow" {
        return Some(if entry.id == "inputNode" {
            "out:output".into()
        } else {
            "in:input".into()
        });
    }
    let definition = &registry.nodes.get(&entry.id)?.definition;
    if matches!(wire, WireType::Image | WireType::Mask) {
        let ports = if incoming {
            &definition.inputs
        } else {
            &definition.outputs
        };
        let first = ports.first()?;
        return Some(format!(
            "{}:{}",
            if incoming { "in" } else { "out" },
            first.name
        ));
    }
    let ports = if incoming {
        &definition.inputs
    } else {
        &definition.outputs
    };
    if let Some(port) = ports.iter().find(|port| port_wire(&port.kind) == wire) {
        return Some(format!(
            "{}:{}",
            if incoming { "in" } else { "out" },
            port.name
        ));
    }
    for param in &definition.params {
        if param.kind == ParamType::Enum {
            continue;
        }
        let Some(param_type) = param_wire(&param.kind) else {
            continue;
        };
        if !bite_core::graph::compatible(param_type, wire) {
            continue;
        }
        // A computed parameter can only supply a wire; a writable one can only receive.
        if !incoming && param.readonly {
            return Some(format!("param:{}", param.name));
        }
        if incoming && !param.readonly && !param.no_port {
            return Some(format!("param:{}", param.name));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(wire: WireType, end: WireEnd) -> PendingWire {
        PendingWire {
            node: "n".into(),
            handle: "out:output".into(),
            end,
            origin: [0.0, 0.0],
            wire,
        }
    }

    fn workflow(id: &str) -> Entry {
        library::workflow_entries()
            .into_iter()
            .find(|entry| entry.id == id)
            .unwrap()
    }

    #[test]
    fn a_value_wire_accepts_every_candidate() {
        let registry = Registry::default();
        let entry = workflow("imageOutputNode");
        assert!(accepts_wire(
            &entry,
            &pending(WireType::Value, WireEnd::Source),
            &registry
        ));
    }

    #[test]
    fn workflow_nodes_only_accept_image_wires() {
        let registry = Registry::default();
        let entry = workflow("imageOutputNode");
        assert!(accepts_wire(
            &entry,
            &pending(WireType::Image, WireEnd::Source),
            &registry
        ));
        assert!(!accepts_wire(
            &entry,
            &pending(WireType::Number, WireEnd::Source),
            &registry
        ));
    }

    #[test]
    fn an_input_is_never_offered_for_a_wire_leaving_a_source() {
        let registry = Registry::default();
        let entry = workflow("inputNode");
        assert!(!accepts_wire(
            &entry,
            &pending(WireType::Image, WireEnd::Source),
            &registry
        ));
        assert!(accepts_wire(
            &entry,
            &pending(WireType::Image, WireEnd::Target),
            &registry
        ));
    }

    #[test]
    fn workflow_auto_connect_picks_the_image_port() {
        let registry = Registry::default();
        assert_eq!(
            first_matching_handle(
                &workflow("imageOutputNode"),
                WireType::Image,
                WireEnd::Target,
                &registry
            ),
            Some("in:input".to_string())
        );
        assert_eq!(
            first_matching_handle(
                &workflow("inputNode"),
                WireType::Image,
                WireEnd::Target,
                &registry
            ),
            Some("out:output".to_string())
        );
    }

    #[test]
    fn opening_the_menu_resets_its_search_and_highlight() {
        let mut menu = CreateMenu {
            query: "old".into(),
            active_index: 4,
            ..CreateMenu::default()
        };
        menu.open_at([10.0, 20.0], None);
        assert!(menu.open);
        assert!(menu.query.is_empty());
        assert_eq!(menu.active_index, 0);
        assert_eq!(menu.position, [10.0, 20.0]);
    }

    #[test]
    fn no_category_flyout_is_open_until_one_is_pointed_at() {
        let mut menu = CreateMenu {
            open_category: Some("Color".into()),
            active_index: 3,
            ..CreateMenu::default()
        };
        menu.open_at([0.0, 0.0], None);
        // The highlight starts on the first row, but its flyout must stay shut: an open
        // flyout would cover the rows below it before the pointer had chosen anything.
        assert_eq!(menu.open_category(), None);
        assert_eq!(menu.active_index, 0);
    }

    #[test]
    fn closing_the_menu_forgets_the_open_flyout() {
        let mut menu = CreateMenu::default();
        menu.open_at([0.0, 0.0], None);
        menu.open_category = Some("Filters".into());
        menu.close();
        assert_eq!(menu.open_category(), None);
        assert!(menu.sub_rect.is_none());
    }

    #[test]
    fn closing_the_menu_drops_the_pending_wire() {
        let mut menu = CreateMenu::default();
        menu.open_at([0.0, 0.0], Some(pending(WireType::Image, WireEnd::Source)));
        assert!(menu.pending.is_some());
        menu.close();
        assert!(!menu.open);
        assert!(menu.pending.is_none());
    }
}
