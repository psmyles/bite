//! The per-node inspectors, one for each `Inspector*Node.svelte` component.
use super::{Edit, InspectorContext, InspectorState, generic, scan_format_groups};
use crate::{
    controls::{self, ButtonKind},
    shell::Rect,
    theme,
};
use bite_imgui::{Color, Rounding, StyleColor, Ui};
use bite_schema::{
    BuiltinNodeKind, GraphNode, NodeKind, ParamValue, ProcessingNodeKind, RenameBlock,
    StructuredParam,
};
use std::collections::BTreeMap;

/// Restricts a command line name to the characters the export accepts.
pub fn sanitize_cli_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                Some(character)
            } else if character.is_ascii_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// The hint beneath a command line name field, and whether it reports a problem.
pub fn cli_name_hint(
    name: &str,
    node_id: &str,
    graph: &bite_schema::Graph,
) -> (String, bool) {
    if name.is_empty() {
        return (
            "No flag (node won't appear in exported script)".into(),
            false,
        );
    }
    let duplicate = graph.nodes.iter().any(|node| {
        node.id != node_id
            && matches!(node.data.params.get("cliName"), Some(ParamValue::String(other)) if other == name)
    });
    if duplicate {
        ("Name already used by another node".into(), true)
    } else {
        (format!("Flag: --{name}"), false)
    }
}

/// True when a Process As Set node appears anywhere upstream of `node_id`.
pub fn upstream_contains(graph: &bite_schema::Graph, node_id: &str, definition_id: &str) -> bool {
    let mut pending = vec![node_id];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(target) = pending.pop() {
        for edge in graph.edges.iter().filter(|edge| edge.target == target) {
            if !visited.insert(edge.source.as_str()) {
                continue;
            }
            if let Some(node) = graph.nodes.iter().find(|node| node.id == edge.source) {
                if node.data.definition_id == definition_id
                    || node.kind == NodeKind::Processing(ProcessingNodeKind::SetInput)
                {
                    return true;
                }
                pending.push(&node.id);
            }
        }
    }
    false
}

/// Dispatches to the right inspector, following the order in `Inspector.svelte`.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    node: &GraphNode,
    state: &mut InspectorState,
    context: &InspectorContext,
) -> Vec<Edit> {
    let width = rect.width();
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => input_node(ui, width, node, state, context),
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            image_output_node(ui, width, node, context)
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            text_output_node(ui, width, node, state, context)
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
            flipbook_output_node(ui, width, node, context)
        }
        NodeKind::Builtin(BuiltinNodeKind::Comment) => comment_node(ui, width, node),
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => folder_path_node(ui, width, node),
        NodeKind::Builtin(BuiltinNodeKind::Group) => group_node(ui, width, node),
        NodeKind::Processing(ProcessingNodeKind::SetInput) => {
            set_input_node(ui, width, node, context)
        }
        _ => match node.data.definition_id.as_str() {
            "rename" => rename_node(ui, width, node, context),
            "resize" => resize_node(ui, width, node, context),
            "format_convert" => format_convert_node(ui, width, node, context),
            _ => generic::draw(ui, width, node, context),
        },
    }
}

/// A padded section with a title, matching the `.section` rule.
fn section(ui: &mut Ui, width: f32, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.dummy([width, 10.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        if !title.is_empty() {
            ui.with_face(theme::face::LABEL, |ui| {
                ui.with_colors(
                    &[(StyleColor::Text, theme::TEXT_BRIGHT.with_alpha(0.6))],
                    |ui| ui.text(title),
                )
            });
            ui.dummy([width - 24.0, 8.0]);
        }
        body(ui);
    });
    ui.dummy([width, 10.0]);
    let origin = ui.cursor_screen_position();
    ui.draw_list().line(
        origin,
        [origin[0] + width, origin[1]],
        theme::NODE_BORDER,
        1.0,
    );
    ui.dummy([width, 1.0]);
}

/// The command line name field shared by every workflow endpoint.
fn cli_name_section(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    placeholder: &str,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    section(ui, width, "CLI Name", |ui| {
        let mut value = match node.data.params.get("cliName") {
            Some(ParamValue::String(text)) => text.clone(),
            _ => String::new(),
        };
        if controls::text_input(ui, "cli-name", &mut value, placeholder, width - 24.0) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "cliName".into(),
                value: ParamValue::String(sanitize_cli_name(&value)),
            });
        }
        ui.dummy([width - 24.0, 4.0]);
        let (hint, is_error) = cli_name_hint(&value, &node.id, context.graph);
        let colour = if is_error {
            theme::COLOR_ERROR_TEXT
        } else {
            theme::TEXT.with_alpha(0.6)
        };
        ui.with_face(theme::face::SMALL_MONO, |ui| {
            ui.with_colors(&[(StyleColor::Text, colour)], |ui| ui.text(&hint))
        });
    });
    edits
}

/// A label on the left with a checkbox against the right edge of the section.
fn checkbox_row(ui: &mut Ui, id: &str, label: &str, width: f32, value: &mut bool) -> bool {
    let origin = ui.cursor_screen_position();
    let height = theme::CHECKBOX_SIZE;
    ui.dummy([width, height]);
    let list = ui.draw_list();
    let text_size = list.measure(theme::face::LABEL, label);
    list.text_with_face(
        [origin[0], origin[1] + (height - text_size[1]) / 2.0],
        theme::TEXT_BRIGHT.with_alpha(0.6),
        theme::face::LABEL,
        label,
    );
    ui.set_cursor_screen_position([origin[0] + width - theme::CHECKBOX_SIZE, origin[1]]);
    let changed = controls::checkbox(ui, id, value);
    ui.set_cursor_screen_position([origin[0], origin[1] + height + 2.0]);
    changed
}

/// A label above a control, used by the sections that are not full parameter rows.
fn field_label(ui: &mut Ui, text: &str) {
    ui.with_face(theme::face::LABEL, |ui| {
        ui.with_colors(
            &[(StyleColor::Text, theme::TEXT_BRIGHT.with_alpha(0.6))],
            |ui| ui.text(text),
        )
    });
    ui.dummy([1.0, 4.0]);
}

/// The Input inspector: naming, thumbnail size, import flow and the loaded file list.
fn input_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    state: &mut InspectorState,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = cli_name_section(ui, width, node, "e.g. input-1", context);

    section(ui, width, "Thumbnail Size", |ui| {
        let options = [128i64, 256, 512, 1024, 2048];
        let labels: Vec<String> = options.iter().map(|size| format!("{size}px")).collect();
        let current = match node.data.params.get("thumbnailSize") {
            Some(ParamValue::Int(value)) => *value,
            Some(ParamValue::Number(value)) => *value as i64,
            _ => 256,
        };
        let mut index = options.iter().position(|size| *size == current).unwrap_or(1);
        if controls::dropdown(ui, "thumb-size", &mut index, &labels, width - 24.0, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "thumbnailSize".into(),
                value: ParamValue::Int(options[index]),
            });
        }
    });

    if context.image_names.is_empty() {
        edits.extend(input_empty_state(ui, width, node, state));
    } else {
        edits.extend(input_loaded_state(ui, width, node, context));
    }
    edits
}

/// The Input inspector before any images are imported.
fn input_empty_state(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    state: &mut InspectorState,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    section(ui, width, "Folder", |ui| {
        let label = if state.scan_folder.is_empty() {
            "Select Input Folder..."
        } else {
            "Change Folder..."
        };
        if controls::button(ui, label, ButtonKind::Neutral, width - 24.0, true) {
            edits.push(Edit::SetInputFolder {
                node: node.id.clone(),
            });
        }

        if !state.scan_folder.is_empty() {
            ui.dummy([width - 24.0, 8.0]);
            folder_card(ui, width - 24.0, &state.scan_folder);
        }

        ui.dummy([width - 24.0, 8.0]);
        let mut recursive = state.scan_recursive;
        if checkbox_row(
            ui,
            "scan-recursive",
            "Include subfolders",
            width - 24.0,
            &mut recursive,
        ) {
            edits.push(Edit::SetScanRecursive {
                node: node.id.clone(),
                value: recursive,
            });
        }

        ui.dummy([width - 24.0, 10.0]);
        field_label(ui, "File formats");
        edits.extend(format_chips(ui, width - 24.0, node, state));

        ui.dummy([width - 24.0, 8.0]);
        let count_text = if state.scanning {
            "Scanning...".to_string()
        } else {
            match state.scan_count {
                Some(count) => format!("{count} file(s) found"),
                None => String::new(),
            }
        };
        if !count_text.is_empty() {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(
                    &[(
                        StyleColor::Text,
                        theme::TEXT.with_alpha(if state.scanning { 0.5 } else { 1.0 }),
                    )],
                    |ui| ui.text(&count_text),
                )
            });
            ui.dummy([width - 24.0, 8.0]);
        }

        let count = state.scan_count.unwrap_or(0);
        let enabled = !state.scanning && count > 0;
        let label = if state.scanning {
            "Importing...".to_string()
        } else {
            format!("Import {count} Image(s)")
        };
        if controls::button(ui, &label, ButtonKind::Primary, width - 24.0, enabled) {
            edits.push(Edit::ImportImages {
                node: node.id.clone(),
            });
        }
    });

    section(ui, width, "Individual Images", |ui| {
        if controls::button(
            ui,
            "Add Individual Images...",
            ButtonKind::Neutral,
            width - 24.0,
            true,
        ) {
            edits.push(Edit::AddIndividualImages {
                node: node.id.clone(),
            });
        }
        ui.dummy([width - 24.0, 10.0]);
        let text = "or drop images onto the filmstrip";
        let size = controls::measure(ui, theme::face::HINT, text);
        let origin = ui.cursor_screen_position();
        ui.draw_list().text_with_face(
            [origin[0] + ((width - 24.0) - size[0]) / 2.0, origin[1]],
            theme::TEXT_BRIGHT.with_alpha(0.5),
            theme::face::HINT,
            text,
        );
        ui.dummy([width - 24.0, size[1]]);
    });
    edits
}

/// The folder card showing the name above the full path.
fn folder_card(ui: &mut Ui, width: f32, path: &str) {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let height = 38.0;
    let origin = ui.cursor_screen_position();
    let list = ui.draw_list();
    let max = [origin[0] + width, origin[1] + height];
    list.rect(origin, max, theme::SEARCH_BG, 3.0, Rounding::All);
    list.rect_outline(origin, max, theme::BORDER, 3.0, Rounding::All, 1.0);
    let name_face = bite_imgui::Face::ui_weight(theme::FONT_SIZE_SM, bite_imgui::Weight::SemiBold);
    controls::draw_ellipsized(
        ui,
        [origin[0] + 7.0, origin[1] + 5.0],
        theme::TEXT_BRIGHT,
        name_face,
        name,
        width - 14.0,
    );
    controls::draw_ellipsized(
        ui,
        [origin[0] + 7.0, origin[1] + 21.0],
        theme::TEXT,
        theme::face::SMALL_MONO,
        path,
        width - 14.0,
    );
    ui.dummy([width, height]);
}

/// The five column grid of format chips.
fn format_chips(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    state: &InspectorState,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let groups = scan_format_groups();
    let columns = 5.0;
    let gap = 4.0;
    let chip_width = (width - gap * (columns - 1.0)) / columns;
    for (index, (name, _)) in groups.iter().enumerate() {
        let active = state.scan_formats.contains(*name);
        let origin = ui.cursor_screen_position();
        let clicked = ui.invisible_button(&format!("##fmt-{name}"), [chip_width, 20.0]);
        let list = ui.draw_list();
        let max = [origin[0] + chip_width, origin[1] + 20.0];
        if active {
            list.rect(
                origin,
                max,
                theme::COLOR_SUCCESS.mix(18.0, theme::PANEL_BG),
                3.0,
                Rounding::All,
            );
        }
        list.rect_outline(
            origin,
            max,
            if active {
                theme::COLOR_SUCCESS.mix(60.0, Color::TRANSPARENT)
            } else {
                theme::BORDER
            },
            3.0,
            Rounding::All,
            1.0,
        );
        let size = list.measure(theme::face::SMALL_MONO, name);
        list.text_with_face(
            [
                origin[0] + (chip_width - size[0]) / 2.0,
                origin[1] + (20.0 - size[1]) / 2.0,
            ],
            if active {
                theme::COLOR_SUCCESS_MUTED
            } else {
                theme::TEXT
            },
            theme::face::SMALL_MONO,
            name,
        );
        // At least one format has to stay enabled.
        if clicked && !(active && state.scan_formats.len() == 1) {
            edits.push(Edit::ToggleScanFormat {
                node: node.id.clone(),
                group: (*name).to_string(),
            });
        }
        if (index + 1) % 5 == 0 {
            ui.dummy([width, gap]);
        } else {
            ui.same_line_at(0.0, gap);
        }
    }
    edits
}

/// The Input inspector once images are loaded.
fn input_loaded_state(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    section(ui, width, "", |ui| {
        if controls::button(ui, "Add Images...", ButtonKind::Neutral, width - 24.0, true) {
            edits.push(Edit::AddIndividualImages {
                node: node.id.clone(),
            });
        }
        ui.dummy([width - 24.0, 8.0]);
        if controls::button(ui, "Clear All", ButtonKind::Danger, width - 24.0, true) {
            edits.push(Edit::ClearImages {
                node: node.id.clone(),
            });
        }
    });

    for (index, name) in context.image_names.iter().enumerate() {
        let height = 24.0;
        let origin = ui.cursor_screen_position();
        let clicked = ui.invisible_button(&format!("##file-{index}"), [width, height]);
        let hovered = ui.item_hovered();
        let list = ui.draw_list();
        if hovered {
            list.rect(
                origin,
                [origin[0] + width, origin[1] + height],
                theme::ACCENT.mix(10.0, theme::PANEL_BG),
                0.0,
                Rounding::None,
            );
        }
        let extension = name
            .rsplit('.')
            .next()
            .filter(|extension| *extension != name.as_str())
            .unwrap_or("")
            .to_uppercase();
        let extension_size = list.measure(theme::face::SMALL_MONO, &extension);
        controls::draw_ellipsized(
            ui,
            [origin[0] + 12.0, origin[1] + 5.0],
            theme::TEXT,
            theme::face::SMALL_MONO,
            name,
            width - 36.0 - extension_size[0],
        );
        list.text_with_face(
            [origin[0] + width - 12.0 - extension_size[0], origin[1] + 5.0],
            theme::TEXT.with_alpha(0.4),
            theme::face::SMALL_MONO,
            &extension,
        );
        if clicked {
            edits.push(Edit::SelectImage {
                node: node.id.clone(),
                index,
            });
        }
    }
    edits
}

/// The Image Output inspector.
fn image_output_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = cli_name_section(ui, width, node, "e.g. output-image-1", context);
    let folder_wired = context
        .graph
        .edges
        .iter()
        .any(|edge| edge.target == node.id && edge.target_handle == "in:folder");

    section(ui, width, "Output Path", |ui| {
        if folder_wired {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::COLOR_SUCCESS_TEXT)], |ui| {
                    ui.text("Using connected folder path")
                })
            });
            return;
        }
        let options = ["source", "custom"];
        let labels: Vec<String> = vec!["Same as source".into(), "Custom folder".into()];
        let current = string_param(node, "outputPath", "source");
        let mut index = options.iter().position(|mode| *mode == current).unwrap_or(0);
        if controls::dropdown(ui, "output-path", &mut index, &labels, width - 24.0, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "outputPath".into(),
                value: ParamValue::String(options[index].into()),
            });
        }
        if options[index] == "custom" {
            ui.dummy([width - 24.0, 8.0]);
            let mut path = string_param(node, "customPath", "");
            let browse_width = 34.0;
            if controls::text_input(
                ui,
                "custom-path",
                &mut path,
                "Enter folder path...",
                width - 24.0 - browse_width - 6.0,
            ) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "customPath".into(),
                    value: ParamValue::String(path),
                });
            }
            ui.same_line_at(0.0, 6.0);
            if controls::button(ui, "...", ButtonKind::Neutral, browse_width, true) {
                edits.push(Edit::BrowseFolder {
                    node: node.id.clone(),
                    name: "customPath".into(),
                });
            }
        }
    });

    edits.extend(overwrite_section(ui, width, node));

    if upstream_contains(context.graph, &node.id, "process_as_set") {
        section(ui, width, "Set Naming", |ui| {
            let half = (width - 24.0 - 8.0) / 2.0;
            ui.group(|ui| {
                field_label(ui, "Prefix");
                let mut value = string_param(node, "setOutputPrefix", "");
                if controls::text_input(ui, "set-prefix", &mut value, "e.g. T_", half) {
                    edits.push(Edit::SetParam {
                        node: node.id.clone(),
                        name: "setOutputPrefix".into(),
                        value: ParamValue::String(value),
                    });
                }
            });
            ui.same_line_at(0.0, 8.0);
            ui.group(|ui| {
                field_label(ui, "Suffix");
                let mut value = string_param(node, "setOutputSuffix", "");
                if controls::text_input(ui, "set-suffix", &mut value, "e.g. _ORM", half) {
                    edits.push(Edit::SetParam {
                        node: node.id.clone(),
                        name: "setOutputSuffix".into(),
                        value: ParamValue::String(value),
                    });
                }
            });
        });
    }

    edits.extend(output_log_section(ui, width, node));
    edits
}

/// The shared overwrite drop-down.
fn overwrite_section(ui: &mut Ui, width: f32, node: &GraphNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    section(ui, width, "Overwrite", |ui| {
        let options = ["skip", "overwrite"];
        let labels: Vec<String> = vec!["Skip existing".into(), "Overwrite".into()];
        let current = string_param(node, "overwrite", "skip");
        let mut index = options.iter().position(|mode| *mode == current).unwrap_or(0);
        if controls::dropdown(ui, "overwrite", &mut index, &labels, width - 24.0, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "overwrite".into(),
                value: ParamValue::String(options[index].into()),
            });
        }
    });
    edits
}

/// The shared log checkbox.
fn output_log_section(ui: &mut Ui, width: f32, node: &GraphNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    section(ui, width, "Output Log", |ui| {
        let mut value = matches!(node.data.params.get("generateLog"), Some(ParamValue::Bool(true)));
        if checkbox_row(
            ui,
            "generate-log",
            "Generate .log file",
            width - 24.0,
            &mut value,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "generateLog".into(),
                value: ParamValue::Bool(value),
            });
        }
    });
    edits
}

/// The Text Output inspector.
fn text_output_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    state: &mut InspectorState,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = cli_name_section(ui, width, node, "e.g. output-text-1", context);

    section(ui, width, "Output File", |ui| {
        let mut path = string_param(node, "outputPath", "");
        let browse_width = 64.0;
        if controls::text_input(
            ui,
            "text-output-path",
            &mut path,
            "path/to/output.txt",
            width - 24.0 - browse_width - 6.0,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "outputPath".into(),
                value: ParamValue::String(path),
            });
        }
        ui.same_line_at(0.0, 6.0);
        if controls::button(ui, "Browse", ButtonKind::Neutral, browse_width, true) {
            edits.push(Edit::BrowseFile {
                node: node.id.clone(),
                name: "outputPath".into(),
                extension: "txt".into(),
            });
        }
    });

    edits.extend(overwrite_section(ui, width, node));

    section(ui, width, "Separator", |ui| {
        let options = ["space", "comma", "tab", "custom"];
        let labels: Vec<String> = vec![
            "Space".into(),
            "Comma".into(),
            "Tab".into(),
            "Custom...".into(),
        ];
        let current = string_param(node, "separatorType", "space");
        let mut index = options
            .iter()
            .position(|option| *option == current)
            .unwrap_or(0);
        if controls::dropdown(ui, "separator", &mut index, &labels, width - 24.0, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "separatorType".into(),
                value: ParamValue::String(options[index].into()),
            });
        }
        if options[index] == "custom" {
            ui.dummy([width - 24.0, 6.0]);
            let mut value = string_param(node, "customSeparator", "");
            if controls::text_input(
                ui,
                "custom-separator",
                &mut value,
                "separator...",
                width - 24.0,
            ) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "customSeparator".into(),
                    value: ParamValue::String(value),
                });
            }
        }
    });

    section(ui, width, "Port Order", |ui| {
        let slots = text_slots(node);
        let connected: Vec<(String, String)> = slots
            .iter()
            .map(|slot| {
                let handle = format!("txo:{slot}");
                let label = context
                    .graph
                    .edges
                    .iter()
                    .find(|edge| edge.target == node.id && edge.target_handle == handle)
                    .and_then(|edge| {
                        context
                            .graph
                            .nodes
                            .iter()
                            .find(|candidate| candidate.id == edge.source)
                    })
                    .map(|source| crate::canvas::view::card_label(source, context.registry))
                    .unwrap_or_else(|| "(unconnected)".into());
                (slot.clone(), label)
            })
            .collect();
        if connected
            .iter()
            .all(|(_, label)| label == "(unconnected)")
        {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(
                    &[(StyleColor::Text, theme::TEXT.with_alpha(0.35))],
                    |ui| ui.text_wrapped("Connect nodes to the Text Output's input ports."),
                )
            });
            return;
        }
        for (slot, label) in &connected {
            let origin = ui.cursor_screen_position();
            ui.dummy([width - 24.0, 22.0]);
            let list = ui.draw_list();
            // Six dots stand in for the drag handle the Svelte list draws.
            for row in 0..3 {
                for column in 0..2 {
                    list.circle(
                        [
                            origin[0] + 2.0 + column as f32 * 4.0,
                            origin[1] + 7.0 + row as f32 * 4.0,
                        ],
                        1.0,
                        theme::TEXT_BRIGHT.with_alpha(0.3),
                    );
                }
            }
            controls::draw_ellipsized(
                ui,
                [origin[0] + 16.0, origin[1] + 4.0],
                theme::TEXT,
                theme::face::SMALL_MONO,
                label,
                width - 48.0,
            );
            let _ = slot;
        }
    });

    edits.extend(output_log_section(ui, width, node));

    section(ui, width, "Processing Source", |ui| {
        let mut value = matches!(
            node.data.params.get("usePreviewForProcessing"),
            Some(ParamValue::Bool(true))
        );
        if checkbox_row(
            ui,
            "use-preview",
            "Use preview image for processing",
            width - 24.0,
            &mut value,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "usePreviewForProcessing".into(),
                value: ParamValue::Bool(value),
            });
        }
    });

    let title = text_preview_title(
        context.image_names.len(),
        state.text_preview.len(),
        state.text_preview_pending,
        text_slots(node).len(),
    );
    section(ui, width, &title, |ui| {
        if state.text_preview_pending {
            controls::hint(ui, "Computing...");
            return;
        }
        if state.text_preview.is_empty() {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::TEXT.with_alpha(0.3))], |ui| {
                    ui.text("Load images to see a preview.")
                })
            });
            return;
        }
        for line in state.text_preview.iter().take(10) {
            controls::draw_ellipsized(
                ui,
                ui.cursor_screen_position(),
                theme::ACCENT,
                theme::face::SMALL_MONO,
                line,
                width - 24.0,
            );
            ui.dummy([width - 24.0, 18.0]);
        }
    });
    edits
}

/// The heading the text preview section shows.
pub fn text_preview_title(
    image_count: usize,
    line_count: usize,
    pending: bool,
    slot_count: usize,
) -> String {
    const LIMIT: usize = 10;
    if slot_count == 0 {
        return "Preview (connect a port)".into();
    }
    if image_count == 0 {
        return "Preview (no files loaded)".into();
    }
    if pending {
        return "Preview...".into();
    }
    if image_count > LIMIT {
        format!("Preview - first {LIMIT} of {image_count} files")
    } else {
        format!("Preview - {line_count} line(s)")
    }
}

/// The Flipbook Output inspector.
fn flipbook_output_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = cli_name_section(ui, width, node, "e.g. output-flipbook-1", context);

    section(ui, width, "Output File", |ui| {
        let mut path = string_param(node, "flipbookOutputPath", "");
        let browse_width = 34.0;
        if controls::text_input(
            ui,
            "atlas-path",
            &mut path,
            "Enter file path...",
            width - 24.0 - browse_width - 6.0,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "flipbookOutputPath".into(),
                value: ParamValue::String(path),
            });
        }
        ui.same_line_at(0.0, 6.0);
        if controls::button(ui, "...", ButtonKind::Neutral, browse_width, true) {
            edits.push(Edit::BrowseFile {
                node: node.id.clone(),
                name: "flipbookOutputPath".into(),
                extension: "png".into(),
            });
        }
    });

    edits.extend(overwrite_section(ui, width, node));

    section(ui, width, "Grid", |ui| {
        let half = (width - 24.0 - 8.0) / 2.0;
        let mut pair = |ui: &mut Ui, left: (&str, &str, i64, i64), right: (&str, &str, i64, i64)| {
            for (index, (name, label, min, max)) in [left, right].into_iter().enumerate() {
                ui.group(|ui| {
                    field_label(ui, label);
                    let current = int_param(node, name, min);
                    let mut text = current.to_string();
                    if controls::text_input(ui, name, &mut text, "", half) {
                        if let Ok(parsed) = text.trim().parse::<i64>() {
                            if parsed >= min && parsed <= max {
                                edits.push(Edit::SetParam {
                                    node: node.id.clone(),
                                    name: name.into(),
                                    value: ParamValue::Int(parsed),
                                });
                            }
                        }
                    }
                });
                if index == 0 {
                    ui.same_line_at(0.0, 8.0);
                }
            }
        };
        pair(
            ui,
            ("cols", "Columns", 1, 64),
            ("rows", "Rows", 1, 64),
        );
        ui.dummy([width - 24.0, 8.0]);
        pair(
            ui,
            ("cellWidth", "Cell width", 1, 4096),
            ("cellHeight", "Cell height", 1, 4096),
        );
    });

    section(ui, width, "Sort Order", |ui| {
        let options = ["import_order", "name", "name_desc"];
        let labels: Vec<String> = vec![
            "Import order".into(),
            "File name (A->Z)".into(),
            "File name (Z->A)".into(),
        ];
        let current = string_param(node, "sortBy", "import_order");
        let mut index = options
            .iter()
            .position(|option| *option == current)
            .unwrap_or(0);
        if controls::dropdown(ui, "sort-by", &mut index, &labels, width - 24.0, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "sortBy".into(),
                value: ParamValue::String(options[index].into()),
            });
        }
    });

    section(ui, width, "Background Color", |ui| {
        let wired = context
            .graph
            .edges
            .iter()
            .any(|edge| edge.target == node.id && edge.target_handle == "param:bgColor");
        if wired {
            controls::badge(ui, "wired", theme::PORT_COLOR_STRING);
            ui.dummy([width - 24.0, 4.0]);
        }
        let stored = node.data.params.get("bgColor");
        let values = generic::vector_of(stored, 4);
        let mut rgba = [
            values[0] as f32,
            values[1] as f32,
            values[2] as f32,
            values.get(3).copied().unwrap_or(1.0) as f32,
        ];
        ui.disabled(wired, |ui| {
            ui.set_next_item_width(width - 24.0);
            if ui.color_edit4("##bg-color", &mut rgba) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "bgColor".into(),
                    value: ParamValue::Vector(
                        rgba.iter().map(|component| *component as f64).collect(),
                    ),
                });
            }
        });
    });

    atlas_summary(ui, width, node, context.image_names.len());
    edits.extend(output_log_section(ui, width, node));
    edits
}

/// The atlas size, cell count and image fit summary.
fn atlas_summary(ui: &mut Ui, width: f32, node: &GraphNode, images: usize) {
    let cols = int_param(node, "cols", 4).max(1);
    let rows = int_param(node, "rows", 4).max(1);
    let cell_width = int_param(node, "cellWidth", 256).max(1);
    let cell_height = int_param(node, "cellHeight", 256).max(1);
    let cells = (cols * rows) as usize;

    let box_width = width - 24.0;
    ui.dummy([width, 8.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    let origin = ui.cursor_screen_position();
    let height = 66.0;
    let list = ui.draw_list();
    let max = [origin[0] + box_width, origin[1] + height];
    list.rect(
        origin,
        max,
        theme::BORDER.mix(20.0, theme::PANEL_BG),
        4.0,
        Rounding::All,
    );
    list.rect_outline(
        origin,
        max,
        theme::BORDER.mix(40.0, Color::TRANSPARENT),
        4.0,
        Rounding::All,
        1.0,
    );

    let row = |index: usize, key: &str, value: &str, warn: bool| {
        let y = origin[1] + 8.0 + index as f32 * 17.0;
        list.text_with_face(
            [origin[0] + 9.0, y],
            theme::TEXT.with_alpha(0.5),
            theme::face::SMALL_MONO,
            key,
        );
        let size = list.measure(theme::face::SMALL_MONO, value);
        list.text_with_face(
            [max[0] - 9.0 - size[0], y],
            if warn {
                theme::COLOR_WARNING_TEXT
            } else {
                theme::TEXT_BRIGHT
            },
            theme::face::SMALL_MONO,
            value,
        );
    };
    row(
        0,
        "Atlas size",
        &format!("{} x {} px", cols * cell_width, rows * cell_height),
        false,
    );
    row(1, "Cells", &format!("{cells} ({cols} x {rows})"), false);
    let (images_text, warn) = if images > cells {
        (
            format!("{images} loaded - {} will be truncated", images - cells),
            true,
        )
    } else if images < cells {
        (
            format!("{images} loaded - {} cells: empty", cells - images),
            true,
        )
    } else {
        (format!("{images} loaded"), false)
    };
    row(2, "Images", &images_text, warn);
    ui.dummy([box_width, height]);
    ui.dummy([width, 8.0]);
}

/// The Folder Path inspector: a single row with a browse control.
fn folder_path_node(ui: &mut Ui, width: f32, node: &GraphNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.dummy([width, 7.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        field_label(ui, "Folder");
        let mut path = string_param(node, "folderPath", "");
        let browse_width = 34.0;
        if controls::text_input(
            ui,
            "folder-path",
            &mut path,
            "Enter folder path...",
            width - 24.0 - browse_width - 6.0,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "folderPath".into(),
                value: ParamValue::String(path),
            });
        }
        ui.same_line_at(0.0, 6.0);
        if controls::button(ui, "...", ButtonKind::Neutral, browse_width, true) {
            edits.push(Edit::BrowseFolder {
                node: node.id.clone(),
                name: "folderPath".into(),
            });
        }
    });
    edits
}

/// The Comment inspector: a heading field and a multi-line body.
fn comment_node(ui: &mut Ui, width: f32, node: &GraphNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.dummy([width, 10.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        field_label(ui, "Heading");
        let mut heading = string_param(node, "heading", "Comment");
        if controls::text_input(ui, "comment-heading", &mut heading, "Heading...", width - 24.0)
        {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "heading".into(),
                value: ParamValue::String(heading),
            });
        }
        ui.dummy([width - 24.0, 12.0]);
        field_label(ui, "Body");
        let mut body = string_param(node, "body", "");
        ui.set_next_item_width(width - 24.0);
        if ui.input_text_multiline(
            "##comment-body",
            &mut body,
            [width - 24.0, 100.0],
            bite_imgui::InputFlags::default(),
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "body".into(),
                value: ParamValue::String(body),
            });
        }
    });
    edits
}

/// The Group inspector, which only names the group.
fn group_node(ui: &mut Ui, width: f32, node: &GraphNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.dummy([width, 10.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        field_label(ui, "Name");
        let mut name = string_param(node, "name", "Group");
        if controls::text_input(ui, "group-name", &mut name, "Group", width - 24.0) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "name".into(),
                value: ParamValue::String(name),
            });
        }
    });
    edits
}

/// The Process As Set inspector: a prefix, the suffix list and the matched set preview.
fn set_input_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let prefix_wired = context
        .graph
        .edges
        .iter()
        .any(|edge| edge.target == node.id && edge.target_handle == "param:prefix");

    ui.dummy([width, 8.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        let label_width = 52.0;
        ui.group(|ui| controls::row_label(ui, "Prefix"));
        ui.same_line_at(label_width, 8.0);
        if prefix_wired {
            controls::badge(ui, "wired", theme::PORT_COLOR_STRING);
        } else {
            let mut prefix = string_param(node, "prefix", "");
            if controls::text_input(
                ui,
                "set-prefix-field",
                &mut prefix,
                "e.g. T_",
                width - 24.0 - label_width - 8.0,
            ) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "prefix".into(),
                    value: ParamValue::String(prefix),
                });
            }
        }
    });

    ui.dummy([width, 10.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    controls::draw_tracked_text(
        ui,
        ui.cursor_screen_position(),
        theme::TEXT_MUTED,
        bite_imgui::Face::ui_weight(theme::FONT_SIZE_XXS, bite_imgui::Weight::SemiBold),
        "SUFFIXES",
        0.07 * 10.0,
    );
    ui.dummy([width, 16.0]);

    let suffixes = set_suffixes(node);
    for (index, suffix) in suffixes.iter().enumerate() {
        ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
        ui.group(|ui| {
            let label_width = 52.0;
            ui.group(|ui| controls::row_label(ui, &format!("suffix {}", index + 1)));
            ui.same_line_at(label_width, 8.0);
            let mut value = suffix.clone();
            let field_width = width - 24.0 - label_width - 8.0 - 36.0;
            if controls::text_input(
                ui,
                &format!("suffix-{index}"),
                &mut value,
                "e.g. _AO",
                field_width,
            ) {
                let mut updated = suffixes.clone();
                updated[index] = value;
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "suffixes".into(),
                    value: ParamValue::Structured(StructuredParam::SetSuffixes {
                        suffixes: updated,
                    }),
                });
            }
            ui.same_line_at(0.0, 6.0);
            if controls::button(ui, "x", ButtonKind::Danger, 30.0, true) {
                let mut updated = suffixes.clone();
                updated.remove(index);
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "suffixes".into(),
                    value: ParamValue::Structured(StructuredParam::SetSuffixes {
                        suffixes: updated,
                    }),
                });
            }
        });
        ui.dummy([width, 6.0]);
    }

    ui.dummy([width, 2.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    if controls::button(ui, "+ Add Suffix", ButtonKind::Neutral, width - 24.0, true) {
        let mut updated = suffixes.clone();
        updated.push(String::new());
        edits.push(Edit::SetParam {
            node: node.id.clone(),
            name: "suffixes".into(),
            value: ParamValue::Structured(StructuredParam::SetSuffixes { suffixes: updated }),
        });
    }

    if suffixes.iter().any(|suffix| !suffix.is_empty()) {
        let prefix = string_param(node, "prefix", "");
        let sets = matched_sets(context.image_names, &prefix, &suffixes);
        let complete = sets
            .iter()
            .filter(|(_, found)| found.iter().all(|present| *present))
            .count();
        section(
            ui,
            width,
            &format!("Matched sets ({complete}/{} complete)", sets.len()),
            |ui| {
                if sets.is_empty() {
                    ui.with_face(theme::face::SMALL_MONO, |ui| {
                        ui.with_colors(
                            &[(StyleColor::Text, theme::TEXT.with_alpha(0.5))],
                            |ui| ui.text("No images match the current pattern."),
                        )
                    });
                    return;
                }
                for (name, found) in sets.iter().take(6) {
                    let all = found.iter().all(|present| *present);
                    let origin = ui.cursor_screen_position();
                    ui.draw_list().line(
                        [origin[0], origin[1]],
                        [origin[0], origin[1] + 18.0],
                        if all {
                            theme::ACCENT
                        } else {
                            theme::COLOR_WARNING.mix(60.0, Color::TRANSPARENT)
                        },
                        2.0,
                    );
                    controls::draw_ellipsized(
                        ui,
                        [origin[0] + 8.0, origin[1] + 2.0],
                        theme::TEXT,
                        theme::face::SMALL_MONO,
                        name,
                        width - 40.0,
                    );
                    ui.dummy([width - 24.0, 20.0]);
                }
                if sets.len() > 6 {
                    ui.with_face(theme::face::SMALL_MONO, |ui| {
                        ui.with_colors(
                            &[(StyleColor::Text, theme::TEXT.with_alpha(0.5))],
                            |ui| ui.text(&format!("...and {} more", sets.len() - 6)),
                        )
                    });
                }
            },
        );
    }
    edits
}

/// Groups file names into sets by the prefix and the suffix list.
pub fn matched_sets(
    names: &[String],
    prefix: &str,
    suffixes: &[String],
) -> Vec<(String, Vec<bool>)> {
    let active: Vec<&String> = suffixes.iter().filter(|s| !s.is_empty()).collect();
    if active.is_empty() {
        return Vec::new();
    }
    let mut groups: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    for name in names {
        let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
        if !stem.starts_with(prefix) {
            continue;
        }
        let remainder = &stem[prefix.len()..];
        let Some((index, suffix)) = active
            .iter()
            .enumerate()
            .find(|(_, suffix)| remainder.ends_with(suffix.as_str()))
        else {
            continue;
        };
        let middle = &remainder[..remainder.len() - suffix.len()];
        let entry = groups
            .entry(format!("{prefix}{middle}"))
            .or_insert_with(|| vec![false; active.len()]);
        entry[index] = true;
    }
    groups.into_iter().collect()
}

/// The Rename inspector: the block list plus a live preview.
fn rename_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let blocks = rename_blocks(node);

    section(ui, width, "Name Blocks", |ui| {
        if blocks.is_empty() {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::TEXT.with_alpha(0.5))], |ui| {
                    ui.text("Add blocks below to build a new filename")
                })
            });
        }
        for (index, block) in blocks.iter().enumerate() {
            let mut updated = blocks.clone();
            let changed = rename_block_row(ui, width - 24.0, index, &mut updated[index]);
            if changed {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "blocks".into(),
                    value: ParamValue::Structured(StructuredParam::RenameBlocks {
                        blocks: updated,
                    }),
                });
            }
            let _ = block;
            ui.dummy([width - 24.0, 6.0]);
        }

        ui.dummy([width - 24.0, 4.0]);
        let third = (width - 24.0 - 12.0) / 3.0;
        let additions: [(&str, RenameBlock); 3] = [
            (
                "+ Text",
                RenameBlock::Text {
                    value: String::new(),
                },
            ),
            (
                "+ Number",
                RenameBlock::Number {
                    start: 1.0,
                    pad: 3.0,
                },
            ),
            (
                "+ Old Name",
                RenameBlock::Oldname {
                    find: String::new(),
                    replace_with: String::new(),
                },
            ),
        ];
        for (index, (label, block)) in additions.into_iter().enumerate() {
            if controls::button(ui, label, ButtonKind::Neutral, third, true) {
                let mut updated = blocks.clone();
                updated.push(block);
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "blocks".into(),
                    value: ParamValue::Structured(StructuredParam::RenameBlocks {
                        blocks: updated,
                    }),
                });
            }
            if index < 2 {
                ui.same_line_at(0.0, 6.0);
            }
        }
    });

    let examples = rename_preview_names(context.image_names);
    let title = if context.image_names.is_empty() {
        "Preview (no files loaded)".to_string()
    } else if context.image_names.len() > 10 {
        format!("Preview - first 10 of {} files", context.image_names.len())
    } else {
        format!("Preview - {} file(s)", context.image_names.len())
    };
    section(ui, width, &title, |ui| {
        let half = (width - 36.0) / 2.0;
        for (index, original) in examples.iter().enumerate() {
            let renamed = bite_core::execution::rename(original, &blocks, index);
            let origin = ui.cursor_screen_position();
            controls::draw_ellipsized(
                ui,
                origin,
                theme::TEXT_BRIGHT.with_alpha(0.5),
                theme::face::SMALL_MONO,
                original,
                half,
            );
            ui.draw_list().text_with_face(
                [origin[0] + half + 2.0, origin[1]],
                theme::TEXT.with_alpha(0.4),
                theme::face::SMALL_MONO,
                "->",
            );
            controls::draw_ellipsized(
                ui,
                [origin[0] + half + 18.0, origin[1]],
                if renamed == *original {
                    theme::ACCENT.with_alpha(0.35)
                } else {
                    theme::ACCENT
                },
                theme::face::SMALL_MONO,
                &renamed,
                half,
            );
            ui.dummy([width - 24.0, 18.0]);
        }
        if context.image_names.is_empty() {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::TEXT.with_alpha(0.4))], |ui| {
                    ui.text("Example filenames shown above")
                })
            });
        }
    });
    edits
}

/// The names the rename preview uses when nothing has been imported.
pub fn rename_preview_names(images: &[String]) -> Vec<String> {
    if images.is_empty() {
        return vec![
            "photo_001.jpg".into(),
            "IMG_5432.jpg".into(),
            "vacation shot.png".into(),
        ];
    }
    images.iter().take(10).cloned().collect()
}

/// One editable rename block. Returns true when the block changed.
fn rename_block_row(ui: &mut Ui, width: f32, index: usize, block: &mut RenameBlock) -> bool {
    let mut changed = false;
    let (label, colour) = match block {
        RenameBlock::Text { .. } => ("TEXT", theme::ACCENT),
        RenameBlock::Number { .. } => ("NUM", theme::COLOR_WARNING),
        RenameBlock::Oldname { .. } => ("ORIG", theme::COLOR_RENAME_ORIG),
    };
    let _id = ui.push_id(&format!("rename-{index}"));
    ui.group(|ui| {
        controls::badge(ui, label, colour);
        ui.same_line_at(0.0, 6.0);
        let field_width = width - 58.0;
        match block {
            RenameBlock::Text { value } => {
                if controls::text_input(ui, "text", value, "text...", field_width) {
                    changed = true;
                }
            }
            RenameBlock::Number { start, pad } => {
                let mut start_text = (*start as i64).to_string();
                if controls::text_input(ui, "start", &mut start_text, "start", 46.0) {
                    if let Ok(parsed) = start_text.trim().parse::<f64>() {
                        if parsed >= 0.0 {
                            *start = parsed;
                            changed = true;
                        }
                    }
                }
                ui.same_line_at(0.0, 6.0);
                let mut pad_text = (*pad as i64).to_string();
                if controls::text_input(ui, "pad", &mut pad_text, "pad", 46.0) {
                    if let Ok(parsed) = pad_text.trim().parse::<f64>() {
                        // The Svelte editor clamps padding to one through eight.
                        if (1.0..=8.0).contains(&parsed) {
                            *pad = parsed;
                            changed = true;
                        }
                    }
                }
            }
            RenameBlock::Oldname { find, replace_with } => {
                let half = (field_width - 22.0) / 2.0;
                if controls::text_input(ui, "find", find, "find...", half) {
                    changed = true;
                }
                if !find.is_empty() {
                    ui.same_line_at(0.0, 4.0);
                    ui.with_face(theme::face::SMALL_MONO, |ui| ui.text("->"));
                    ui.same_line_at(0.0, 4.0);
                    if controls::text_input(ui, "replace", replace_with, "replace...", half) {
                        changed = true;
                    }
                }
            }
        }
    });
    changed
}

/// The Resize inspector, with its dimension preview.
fn resize_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    // The generic editor already covers every Resize parameter and its visibility rules.
    generic::draw(ui, width, node, context)
}

/// The Convert Format inspector: the format choice then that format's own parameters.
fn format_convert_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
) -> Vec<Edit> {
    let mut edits = generic::draw(ui, width, node, context);
    let format = string_param(node, "format", "PNG").to_uppercase();
    let Some(definition) = context.registry.formats.get(&format) else {
        return edits;
    };
    if definition.0.params.is_empty() {
        ui.dummy([width, 10.0]);
        ui.set_cursor_screen_position([
            ui.cursor_screen_position()[0] + 12.0,
            ui.cursor_screen_position()[1],
        ]);
        ui.with_face(theme::face::LABEL, |ui| {
            ui.with_colors(&[(StyleColor::Text, theme::TEXT.with_alpha(0.4))], |ui| {
                ui.text("No encoding options for this format.")
            })
        });
        return edits;
    }
    for parameter in &definition.0.params {
        edits.extend(generic::row(ui, width, node, parameter, false, context));
    }
    edits
}

// -- Parameter readers ---------------------------------------------------------------

fn string_param(node: &GraphNode, name: &str, fallback: &str) -> String {
    match node.data.params.get(name) {
        Some(ParamValue::String(text)) => text.clone(),
        _ => fallback.to_string(),
    }
}

fn int_param(node: &GraphNode, name: &str, fallback: i64) -> i64 {
    match node.data.params.get(name) {
        Some(ParamValue::Int(value)) => *value,
        Some(ParamValue::Number(value)) => *value as i64,
        _ => fallback,
    }
}

fn set_suffixes(node: &GraphNode) -> Vec<String> {
    match node.data.params.get("suffixes") {
        Some(ParamValue::Structured(StructuredParam::SetSuffixes { suffixes })) => suffixes.clone(),
        _ => Vec::new(),
    }
}

fn text_slots(node: &GraphNode) -> Vec<String> {
    match node.data.params.get("portIds") {
        Some(ParamValue::Structured(StructuredParam::TextSlots { slots })) => slots.clone(),
        _ => Vec::new(),
    }
}

fn rename_blocks(node: &GraphNode) -> Vec<RenameBlock> {
    match node.data.params.get("blocks") {
        Some(ParamValue::Structured(StructuredParam::RenameBlocks { blocks })) => blocks.clone(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{GraphEdge, NodeData, Position, Viewport};

    fn node_with(id: &str, params: Vec<(&str, ParamValue)>) -> GraphNode {
        let mut data = NodeData {
            label: String::new(),
            definition_id: "resize".into(),
            params: Default::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        for (name, value) in params {
            data.params.insert(name.into(), value);
        }
        GraphNode {
            id: id.into(),
            kind: NodeKind::Builtin(BuiltinNodeKind::Input),
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data,
        }
    }

    fn graph(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> bite_schema::Graph {
        bite_schema::Graph {
            nodes,
            edges,
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }

    #[test]
    fn command_line_names_keep_only_the_accepted_characters() {
        assert_eq!(sanitize_cli_name("Output Image 1"), "output-image-1");
        assert_eq!(sanitize_cli_name("a_b!c"), "abc");
        assert_eq!(sanitize_cli_name("Already-Fine"), "already-fine");
    }

    #[test]
    fn an_empty_command_line_name_reports_that_no_flag_is_exported() {
        let graph = graph(vec![node_with("a", vec![])], Vec::new());
        let (hint, error) = cli_name_hint("", "a", &graph);
        assert_eq!(hint, "No flag (node won't appear in exported script)");
        assert!(!error);
    }

    #[test]
    fn a_duplicate_command_line_name_is_reported_as_an_error() {
        let graph = graph(
            vec![
                node_with("a", vec![("cliName", ParamValue::String("input-1".into()))]),
                node_with("b", vec![("cliName", ParamValue::String("input-1".into()))]),
            ],
            Vec::new(),
        );
        let (hint, error) = cli_name_hint("input-1", "b", &graph);
        assert_eq!(hint, "Name already used by another node");
        assert!(error);
    }

    #[test]
    fn a_unique_command_line_name_reports_its_flag() {
        let graph = graph(
            vec![node_with(
                "a",
                vec![("cliName", ParamValue::String("input-1".into()))],
            )],
            Vec::new(),
        );
        let (hint, error) = cli_name_hint("input-1", "a", &graph);
        assert_eq!(hint, "Flag: --input-1");
        assert!(!error);
    }

    #[test]
    fn matched_sets_group_by_the_middle_segment() {
        let names = vec![
            "T_rock_AO.png".to_string(),
            "T_rock_N.png".to_string(),
            "T_wood_AO.png".to_string(),
        ];
        let suffixes = vec!["_AO".to_string(), "_N".to_string()];
        let sets = matched_sets(&names, "T_", &suffixes);
        assert_eq!(sets.len(), 2);
        let rock = sets.iter().find(|(name, _)| name == "T_rock").unwrap();
        assert_eq!(rock.1, vec![true, true]);
        let wood = sets.iter().find(|(name, _)| name == "T_wood").unwrap();
        assert_eq!(wood.1, vec![true, false]);
    }

    #[test]
    fn files_outside_the_prefix_are_not_matched() {
        let names = vec!["X_rock_AO.png".to_string()];
        let suffixes = vec!["_AO".to_string()];
        assert!(matched_sets(&names, "T_", &suffixes).is_empty());
    }

    #[test]
    fn the_rename_preview_falls_back_to_example_names() {
        let examples = rename_preview_names(&[]);
        assert_eq!(examples.len(), 3);
        assert_eq!(examples[0], "photo_001.jpg");
        let real: Vec<String> = (0..20).map(|index| format!("file{index}.png")).collect();
        assert_eq!(rename_preview_names(&real).len(), 10);
    }

    #[test]
    fn the_text_preview_title_reports_each_state() {
        assert_eq!(
            text_preview_title(0, 0, false, 0),
            "Preview (connect a port)"
        );
        assert_eq!(
            text_preview_title(0, 0, false, 1),
            "Preview (no files loaded)"
        );
        assert_eq!(text_preview_title(3, 0, true, 1), "Preview...");
        assert_eq!(text_preview_title(3, 3, false, 1), "Preview - 3 line(s)");
        assert_eq!(
            text_preview_title(42, 10, false, 1),
            "Preview - first 10 of 42 files"
        );
    }

    #[test]
    fn a_process_as_set_upstream_is_detected_through_the_chain() {
        let mut set_node = node_with("s", vec![]);
        set_node.kind = NodeKind::Processing(ProcessingNodeKind::SetInput);
        let middle = node_with("m", vec![]);
        let output = node_with("o", vec![]);
        let edges = vec![
            GraphEdge {
                id: "e1".into(),
                source: "s".into(),
                source_handle: "out:suffix_0".into(),
                target: "m".into(),
                target_handle: "in:input".into(),
            },
            GraphEdge {
                id: "e2".into(),
                source: "m".into(),
                source_handle: "out:output".into(),
                target: "o".into(),
                target_handle: "in:input".into(),
            },
        ];
        let graph = graph(vec![set_node, middle, output], edges);
        assert!(upstream_contains(&graph, "o", "process_as_set"));
        assert!(!upstream_contains(&graph, "s", "process_as_set"));
    }
}
