//! The Node Library panel, from `src/renderer/components/NodeLibrary.svelte`.
use crate::{
    controls::{self, HoverTimer},
    shell::Rect,
    theme,
};
use bite_core::Registry;
use bite_imgui::{MouseCursor, Rounding, StyleColor, StyleVar, Ui, WindowFlags};
use std::collections::BTreeSet;

/// One entry the library can offer, whether a built-in or a definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The payload used for drag and drop and for creation.
    pub id: String,
    pub label: String,
    pub category: String,
    pub description: String,
    pub aliases: Vec<String>,
}

/// The four built-in workflow entries, pinned above the definition categories.
pub fn workflow_entries() -> Vec<Entry> {
    [
        ("inputNode", "Input", "Source of images for the workflow"),
        (
            "imageOutputNode",
            "Image Output",
            "Write processed images to disk",
        ),
        (
            "textOutputNode",
            "Text Output",
            "Write text/metadata values to a file",
        ),
        (
            "flipbookOutputNode",
            "Flipbook Output",
            "Assemble images into a flipbook atlas",
        ),
    ]
    .into_iter()
    .map(|(id, label, description)| Entry {
        id: id.into(),
        label: label.into(),
        category: "Workflow".into(),
        description: description.into(),
        aliases: Vec::new(),
    })
    .collect()
}

/// Every definition in the registry, as library entries.
pub fn definition_entries(registry: &Registry) -> Vec<Entry> {
    let mut entries: Vec<Entry> = registry
        .nodes
        .values()
        .map(|node| Entry {
            id: node.definition.id.clone(),
            label: node.definition.label.clone(),
            category: node.definition.category.clone(),
            description: node.definition.description.clone(),
            aliases: node.definition.aliases.clone(),
        })
        .collect();
    entries.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
    entries
}

/// True when the entry matches the filter, which is a case-insensitive substring test over
/// the label, the category and the aliases.
pub fn matches(entry: &Entry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    entry.label.to_lowercase().contains(&query)
        || entry.category.to_lowercase().contains(&query)
        || entry
            .aliases
            .iter()
            .any(|alias| alias.to_lowercase().contains(&query))
}

/// Groups matching entries by category, with both levels sorted by label.
/// Orders two names the way `localeCompare` does: by case-insensitive text, so `Filters`,
/// `Format` and `FX` fall in that order rather than having `FX` jump ahead on its capital.
fn locale_compare(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

pub fn grouped(entries: &[Entry], query: &str) -> Vec<(String, Vec<Entry>)> {
    let mut categories: std::collections::BTreeMap<String, Vec<Entry>> = Default::default();
    for entry in entries.iter().filter(|entry| matches(entry, query)) {
        categories
            .entry(entry.category.clone())
            .or_default()
            .push(entry.clone());
    }
    let mut out: Vec<(String, Vec<Entry>)> = categories.into_iter().collect();
    for (_, items) in &mut out {
        items.sort_by(|a, b| locale_compare(&a.label, &b.label));
    }
    out.sort_by(|a, b| locale_compare(&a.0, &b.0));
    out
}

/// State the panel keeps between frames.
#[derive(Default)]
pub struct LibraryState {
    pub query: String,
    pub collapsed: BTreeSet<String>,
    pub workflow_collapsed: bool,
    pub tooltip: HoverTimer,
}

impl LibraryState {
    /// While a search is active every group is forced open, as the Svelte panel does.
    fn is_open(&self, category: &str) -> bool {
        if !self.query.trim().is_empty() {
            return true;
        }
        !self.collapsed.contains(category)
    }

    fn toggle(&mut self, category: &str) {
        if !self.collapsed.remove(category) {
            self.collapsed.insert(category.to_string());
        }
    }
}

/// Draws the panel. Returns the entry to add when one was clicked.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    state: &mut LibraryState,
    registry: &Registry,
    delta: f32,
) -> Option<Entry> {
    let mut chosen = None;
    ui.set_next_window_position(rect.min);
    ui.set_next_window_size(rect.size());
    let mut flags = WindowFlags::panel();
    flags.no_background = true;
    flags.no_scrollbar = true;

    ui.window_with("##library", flags, |ui| {
        ui.draw_list().rect(
            rect.min,
            rect.max,
            theme::PANEL_BG,
            theme::PANEL_RADIUS,
            Rounding::All,
        );
        ui.set_cursor_screen_position(rect.min);
        controls::panel_header(ui, "Node Library", None);

        // The search strip shares the header background.
        let search_origin = ui.cursor_screen_position();
        ui.draw_list().rect(
            search_origin,
            [rect.max[0], search_origin[1] + 42.0],
            theme::PANEL_HEADER_BG,
            0.0,
            Rounding::None,
        );
        ui.set_cursor_screen_position([search_origin[0] + 10.0, search_origin[1] + 8.0]);
        controls::search_input(
            ui,
            "library-filter",
            &mut state.query,
            "Filter nodes...",
            rect.width() - 20.0,
        );
        ui.set_cursor_screen_position([search_origin[0], search_origin[1] + 42.0 + 4.0]);

        let searching = !state.query.trim().is_empty();
        let workflow = workflow_entries();
        let definitions = definition_entries(registry);
        let groups = grouped(&definitions, &state.query);
        let matched_workflow: Vec<Entry> = workflow
            .iter()
            .filter(|entry| matches(entry, &state.query))
            .cloned()
            .collect();

        if groups.is_empty() && (searching || definitions.is_empty()) && matched_workflow.is_empty()
        {
            ui.set_cursor_screen_position([rect.min[0] + 12.0, ui.cursor_screen_position()[1] + 12.0]);
            controls::hint(
                ui,
                if searching {
                    "No matching nodes."
                } else {
                    "No nodes loaded."
                },
            );
            return;
        }

        let body_height = (rect.max[1] - ui.cursor_screen_position()[1] - 8.0).max(1.0);
        ui.with_style(&[StyleVar::ItemSpacing([0.0, 0.0])], |ui| {
            ui.child("library-scroll", [rect.width(), body_height], false, |ui| {
                // The Workflow section is pinned first and hides while searching.
                if !searching {
                    let open = !state.workflow_collapsed;
                    if category_row(ui, "Workflow", open, rect.width()) {
                        state.workflow_collapsed = !state.workflow_collapsed;
                    }
                    if open {
                        for entry in &workflow {
                            if entry_row(ui, entry, rect.width(), state, delta) {
                                chosen = Some(entry.clone());
                            }
                        }
                    }
                }
                for (category, entries) in &groups {
                    let open = state.is_open(category);
                    if category_row(ui, category, open, rect.width()) {
                        state.toggle(category);
                    }
                    if open {
                        for entry in entries {
                            if entry_row(ui, entry, rect.width(), state, delta) {
                                chosen = Some(entry.clone());
                            }
                        }
                    }
                }
                ui.dummy([rect.width(), 8.0]);
            });
        });
    });
    chosen
}

/// A collapsible category heading with the minus and plus glyphs the panel uses.
fn category_row(ui: &mut Ui, label: &str, open: bool, width: f32) -> bool {
    let height = 22.0;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##cat-{label}"), [width - 12.0, height]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    let colour = if hovered {
        theme::TEXT
    } else {
        theme::TEXT_BRIGHT
    };
    // `.category-label` centres its glyph and its text on the row, and `.collapse-icon`
    // centres the glyph again inside a ten pixel box, with the panel gap after it.
    let glyph = if open { "-" } else { "+" };
    let glyph_face = theme::face::VALUE;
    let glyph_size = controls::measure(ui, glyph_face, glyph);
    let label_size = controls::measure(ui, theme::face::CATEGORY_LABEL, label);
    let icon_left = origin[0] + 12.0;
    let list = ui.draw_list();
    list.text_with_face(
        [
            icon_left + (theme::LIBRARY_COLLAPSE_ICON_WIDTH - glyph_size[0]) / 2.0,
            origin[1] + (height - glyph_size[1]) / 2.0,
        ],
        colour,
        glyph_face,
        glyph,
    );
    controls::draw_tracked_text(
        ui,
        [
            icon_left + theme::LIBRARY_COLLAPSE_ICON_WIDTH + theme::PANEL_GAP,
            origin[1] + (height - label_size[1]) / 2.0,
        ],
        colour,
        theme::face::CATEGORY_LABEL,
        label,
        0.04 * f32::from(theme::FONT_SIZE_BASE),
    );
    clicked
}

/// One draggable library entry.
fn entry_row(
    ui: &mut Ui,
    entry: &Entry,
    width: f32,
    state: &mut LibraryState,
    delta: f32,
) -> bool {
    let height = 20.0;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##entry-{}", entry.id), [width - 12.0, height]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    let list = ui.draw_list();
    if hovered {
        list.rect(
            [origin[0] + 6.0, origin[1]],
            [origin[0] + width - 6.0, origin[1] + height],
            theme::LIBRARY_ITEM_HOVER_BG,
            3.0,
            Rounding::All,
        );
    }
    controls::draw_ellipsized(
        ui,
        [origin[0] + 12.0, origin[1] + 3.0],
        theme::TEXT,
        theme::face::NODE_ITEM,
        &entry.label,
        width - 44.0,
    );
    // The stylesheet marks a draggable row with three stacked colons.
    list.text_with_face(
        [origin[0] + width - 28.0, origin[1] + 2.0],
        theme::TEXT_BRIGHT.with_alpha(0.5),
        theme::face::COMMENT_BODY,
        ":::",
    );

    ui.drag_source("bite-node", &entry.id, |ui| {
        ui.with_face(theme::face::NODE_ITEM, |ui| {
            ui.with_colors(&[(StyleColor::Text, theme::TEXT_BRIGHT)], |ui| {
                ui.text(&entry.label)
            })
        });
    });

    if !entry.description.is_empty()
        && state
            .tooltip
            .poll(&entry.id, hovered, delta, theme::TOOLTIP_DELAY_MENU)
    {
        draw_tooltip(ui, &entry.description);
    }
    clicked
}

/// The shared hover tooltip, styled after the `.node-tooltip-fixed` rule.
pub fn draw_tooltip(ui: &mut Ui, text: &str) {
    ui.with_style(
        &[
            StyleVar::WindowPadding([9.0, 5.0]),
            StyleVar::WindowRounding(4.0),
            StyleVar::PopupBorderSize(1.0),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::PopupBg, theme::BG.with_alpha(0.92)),
                    (StyleColor::Border, theme::NODE_BORDER),
                    (StyleColor::Text, theme::TEXT_BRIGHT),
                ],
                |ui| {
                    ui.tooltip(|ui| {
                        ui.with_face(theme::face::LABEL, |ui| {
                            // The width is named explicitly: an auto-sized tooltip has no
                            // edge to wrap against until its content has been measured.
                            ui.text_wrapped_at(text, theme::TOOLTIP_WIDTH);
                        });
                    })
                },
            )
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_sort_the_way_locale_compare_does() {
        // A capital letter must not jump ahead of a lower-case one: `FX` belongs after
        // `Format`, which a plain byte comparison gets wrong.
        let mut names = vec!["FX", "Filters", "Format", "Color"];
        names.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(names, vec!["Color", "Filters", "Format", "FX"]);
    }

    fn entry(label: &str, category: &str, aliases: &[&str]) -> Entry {
        Entry {
            id: label.to_lowercase().replace(' ', "_"),
            label: label.into(),
            category: category.into(),
            description: String::new(),
            aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
        }
    }

    #[test]
    fn the_filter_matches_labels_categories_and_aliases() {
        let resize = entry("Resize", "Transform", &["scale"]);
        assert!(matches(&resize, "res"));
        assert!(matches(&resize, "TRANSFORM"));
        assert!(matches(&resize, "scale"));
        assert!(!matches(&resize, "blur"));
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        assert!(matches(&entry("Resize", "Transform", &[]), "   "));
    }

    #[test]
    fn groups_are_sorted_by_category_then_label() {
        let entries = vec![
            entry("Sharpen", "Filters", &[]),
            entry("Resize", "Transform", &[]),
            entry("Blur", "Filters", &[]),
        ];
        let groups = grouped(&entries, "");
        assert_eq!(groups[0].0, "Filters");
        assert_eq!(groups[0].1[0].label, "Blur");
        assert_eq!(groups[0].1[1].label, "Sharpen");
        assert_eq!(groups[1].0, "Transform");
    }

    #[test]
    fn a_search_forces_every_category_open() {
        let mut state = LibraryState::default();
        state.collapsed.insert("Transform".into());
        assert!(!state.is_open("Transform"));
        state.query = "res".into();
        assert!(state.is_open("Transform"));
    }

    #[test]
    fn the_workflow_section_offers_the_four_built_ins() {
        let entries = workflow_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].id, "inputNode");
        assert!(entries.iter().all(|entry| entry.category == "Workflow"));
    }
}
