//! The Inspector panel, from `src/renderer/components/Inspector.svelte` and its family of
//! per-node editors.
pub mod generic;
pub mod nodes;

use crate::{
    controls::{self, ButtonKind},
    shell::Rect,
    theme,
};
use bite_core::Registry;
use bite_imgui::{Rounding, StyleVar, Ui, WindowFlags};
use bite_schema::{BuiltinNodeKind, GraphNode, NodeKind, ParamValue, ProcessingNodeKind};

/// An edit the inspector asks the application to apply.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    SetParam {
        node: String,
        name: String,
        value: ParamValue,
    },
    /// A per-node run folder, which is not part of the graph.
    SetRuntimePath {
        node: String,
        path: String,
    },
    ImportImages {
        node: String,
    },
    AddIndividualImages {
        node: String,
    },
    ClearImages {
        node: String,
    },
    SelectImage {
        node: String,
        index: usize,
    },
    RunWorkflow,
    /// A browse button was pressed for a folder-valued parameter.
    BrowseFolder {
        node: String,
        name: String,
    },
    /// A browse button was pressed for a file-valued parameter.
    BrowseFile {
        node: String,
        name: String,
        extension: String,
    },
    /// Scanning options for the Input empty state, which live outside the graph.
    SetScanRecursive {
        node: String,
        value: bool,
    },
    ToggleScanFormat {
        node: String,
        group: String,
    },
    SetInputFolder {
        node: String,
    },
}

/// The title the header shows on the right, matching `nodeLabel`.
pub fn node_label(node: &GraphNode, registry: &Registry) -> String {
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => {
            if node.data.label.is_empty() {
                "Input".into()
            } else {
                node.data.label.clone()
            }
        }
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => "Image Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => "Text Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => "Flipbook Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::Comment) => "Comment".into(),
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => "Folder Path".into(),
        NodeKind::Builtin(BuiltinNodeKind::Group) => "Group".into(),
        NodeKind::Processing(ProcessingNodeKind::SetInput) => "Process As Set".into(),
        _ => registry
            .nodes
            .get(&node.data.definition_id)
            .map(|entry| entry.definition.label.clone())
            .unwrap_or_else(|| node.data.label.clone()),
    }
}

/// Everything the inspector needs beyond the selected node.
pub struct InspectorContext<'a> {
    pub registry: &'a Registry,
    pub graph: &'a bite_schema::Graph,
    /// Live values from the preview run for this node.
    pub resolved: Option<&'a std::collections::BTreeMap<String, ParamValue>>,
    /// File names imported for the selected Input branch, for the previews.
    pub image_names: &'a [String],
    /// The per-node run folder overrides.
    pub runtime_paths: &'a std::collections::BTreeMap<String, String>,
    /// True while a batch is running, which disables the run action.
    pub running: bool,
    /// The run button's tooltip and whether it can be pressed.
    pub run_ready: bool,
    pub run_tooltip: String,
    pub delta: f32,
}

/// State the inspector keeps between frames, such as scan options and edit buffers.
#[derive(Default)]
pub struct InspectorState {
    pub scan_recursive: bool,
    pub scan_formats: std::collections::BTreeSet<String>,
    pub scan_folder: String,
    pub scan_count: Option<usize>,
    pub scanning: bool,
    pub text_preview: Vec<String>,
    pub text_preview_pending: bool,
    pub tooltip: controls::HoverTimer,
}

impl InspectorState {
    /// The formats enabled by default, from the Input inspector's chip grid.
    pub fn with_default_formats() -> Self {
        let mut state = Self::default();
        for group in ["PNG", "JPEG", "WEBP", "TIFF"] {
            state.scan_formats.insert(group.to_string());
        }
        state
    }
}

/// The format chips the Input inspector offers, with the extensions each covers.
pub fn scan_format_groups() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("PNG", vec!["png"]),
        ("JPEG", vec!["jpg", "jpeg"]),
        ("WEBP", vec!["webp"]),
        ("AVIF", vec!["avif"]),
        ("TIFF", vec!["tif", "tiff"]),
        ("BMP", vec!["bmp"]),
        ("TGA", vec!["tga"]),
        ("PSD", vec!["psd", "psb"]),
        ("EXR", vec!["exr", "hdr"]),
        (
            "RAW",
            vec![
                "cr2", "cr3", "nef", "nrw", "arw", "dng", "orf", "raf", "rw2", "pef", "srw",
            ],
        ),
    ]
}

/// Draws the panel and returns every edit raised this frame.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    selected: Option<&GraphNode>,
    state: &mut InspectorState,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.set_next_window_position(rect.min);
    ui.set_next_window_size(rect.size());
    let mut flags = WindowFlags::panel();
    flags.no_background = true;
    flags.no_scrollbar = true;

    ui.window_with("##inspector", flags, |ui| {
        ui.draw_list().rect(
            rect.min,
            rect.max,
            theme::PANEL_BG,
            theme::PANEL_RADIUS,
            Rounding::All,
        );
        ui.set_cursor_screen_position(rect.min);
        let title = selected.map(|node| node_label(node, context.registry));
        controls::panel_header(ui, "Inspector", title.as_deref());

        let footer_height = 51.0;
        let body_height = (rect.height() - theme::PANEL_HEADER_HEIGHT - footer_height).max(1.0);
        ui.with_style(&[StyleVar::ItemSpacing([0.0, 0.0])], |ui| {
            ui.child("inspector-body", [rect.width(), body_height], false, |ui| {
                match selected {
                    None => {
                        ui.dummy([rect.width(), 14.0]);
                        ui.set_cursor_screen_position([
                            rect.min[0] + 12.0,
                            ui.cursor_screen_position()[1],
                        ]);
                        controls::hint(ui, "Select a node to edit its parameters.");
                    }
                    Some(node) => {
                        edits.extend(nodes::draw(ui, rect, node, state, context));
                    }
                }
            });
        });

        // The primary action sits in a bordered footer at the bottom of the panel.
        let footer_top = rect.max[1] - footer_height;
        ui.draw_list().line(
            [rect.min[0], footer_top],
            [rect.max[0], footer_top],
            theme::BORDER,
            1.0,
        );
        ui.set_cursor_screen_position([rect.min[0] + 12.0, footer_top + 10.0]);
        let enabled = context.run_ready && !context.running;
        if controls::button(
            ui,
            "Run Workflow",
            ButtonKind::Primary,
            rect.width() - 24.0,
            enabled,
        ) {
            edits.push(Edit::RunWorkflow);
        }
        if ui.item_hovered() && !context.run_tooltip.is_empty() {
            library_tooltip(ui, &context.run_tooltip);
        }
    });
    edits
}

fn library_tooltip(ui: &mut Ui, text: &str) {
    crate::panels::library::draw_tooltip(ui, text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{NodeData, Position};

    fn node(kind: NodeKind, definition: &str) -> GraphNode {
        GraphNode {
            id: "n".into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: String::new(),
                definition_id: definition.into(),
                params: Default::default(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        }
    }

    #[test]
    fn built_in_nodes_name_themselves_in_the_header() {
        let registry = Registry::default();
        assert_eq!(
            node_label(
                &node(NodeKind::Builtin(BuiltinNodeKind::TextOutput), ""),
                &registry
            ),
            "Text Output"
        );
        assert_eq!(
            node_label(
                &node(NodeKind::Processing(ProcessingNodeKind::SetInput), ""),
                &registry
            ),
            "Process As Set"
        );
    }

    #[test]
    fn the_default_scan_formats_match_the_input_inspector() {
        let state = InspectorState::with_default_formats();
        assert!(state.scan_formats.contains("PNG"));
        assert!(state.scan_formats.contains("JPEG"));
        assert!(state.scan_formats.contains("WEBP"));
        assert!(state.scan_formats.contains("TIFF"));
        assert!(!state.scan_formats.contains("AVIF"));
        assert_eq!(state.scan_formats.len(), 4);
    }

    #[test]
    fn the_format_chip_grid_offers_ten_groups() {
        let groups = scan_format_groups();
        assert_eq!(groups.len(), 10);
        assert!(groups.iter().any(|(name, _)| *name == "RAW"));
        let jpeg = groups.iter().find(|(name, _)| *name == "JPEG").unwrap();
        assert_eq!(jpeg.1, vec!["jpg", "jpeg"]);
    }
}
