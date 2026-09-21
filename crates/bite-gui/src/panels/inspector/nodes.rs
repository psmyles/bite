//! The per-node inspectors, one for each `Inspector*Node.svelte` component.
use super::{Edit, InspectorContext, InspectorState, generic, scan_format_groups};
use crate::{
    color_picker,
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
            flipbook_output_node(ui, width, node, context, &mut state.pickers)
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
            "format_convert" => {
                format_convert_node(ui, width, node, context, &mut state.pickers)
            }
            _ => generic::draw(ui, width, node, context, &mut state.pickers),
        },
    }
}

/// `.field-input` is shorter than the thirty-pixel `.text-input`: eleven-pixel mono inside
/// four pixels of padding and a one-pixel border.
const COMPACT_INPUT_HEIGHT: f32 = 24.0;
/// The width `.block-delete` takes, its fifteen-pixel cross inside two pixels a side.
const DELETE_WIDTH: f32 = 19.0;

/// A padded section with a title, matching the `.section` rule.
fn section(ui: &mut Ui, width: f32, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.dummy([width, 10.0]);
    ui.set_cursor_screen_position([ui.cursor_screen_position()[0] + 12.0, ui.cursor_screen_position()[1]]);
    ui.group(|ui| {
        if !title.is_empty() {
            ui.with_face(theme::face::LABEL, |ui| {
                ui.with_colors(
                    &[(StyleColor::Text, theme::INSPECTOR_LABEL)],
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
/// The gap a checkbox row leaves below itself.
const ROW_GAP: f32 = 2.0;

fn checkbox_row(ui: &mut Ui, id: &str, label: &str, width: f32, value: &mut bool) -> bool {
    let origin = ui.cursor_screen_position();
    let height = theme::CHECKBOX_SIZE;
    // The row reserves its own height *and* the two pixels of gap it leaves behind it, because
    // the last thing it does is put the cursor there. Reserving only the height and then moving
    // past it leaves the cursor outside the parent's content extent with no item to grow it,
    // which since 1.92 Dear ImGui reports as the layout error it has always been.
    ui.dummy([width, height + ROW_GAP]);
    let list = ui.draw_list();
    let text_size = list.measure(theme::face::LABEL, label);
    // `.log-toggle` and `.checkbox-label` take the bright text at full strength. This is a
    // thing the row says, not a name for something below it, so it must not read as one of
    // the section titles above it, which are the same colour at six tenths.
    list.text_with_face(
        [origin[0], origin[1] + (height - text_size[1]) / 2.0],
        theme::INSPECTOR_VALUE,
        theme::face::LABEL,
        label,
    );
    ui.set_cursor_screen_position([origin[0] + width - theme::CHECKBOX_SIZE, origin[1]]);
    let changed = controls::checkbox(ui, id, value);
    ui.set_cursor_screen_position([origin[0], origin[1] + height + ROW_GAP]);
    changed
}

/// The tooltip a `title` attribute would give the last item drawn.
fn title_tooltip(ui: &mut Ui, text: &str) {
    if ui.item_hovered_after_delay() {
        crate::panels::library::draw_tooltip(ui, text);
    }
}

/// A label above a control, used by the sections that are not full parameter rows.
fn field_label(ui: &mut Ui, text: &str) {
    ui.with_face(theme::face::LABEL, |ui| {
        ui.with_colors(
            &[(StyleColor::Text, theme::INSPECTOR_LABEL)],
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
            let browse = controls::button(ui, "...", ButtonKind::Neutral, browse_width, true);
        title_tooltip(ui, "Browse...");
        if browse {
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
            // With images loaded there is nothing to show only because no port is wired.
            let hint = if context.image_names.is_empty() {
                "Load images to see a preview."
            } else {
                "Connect at least one port to see a preview."
            };
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::TEXT.with_alpha(0.3))], |ui| {
                    ui.text(hint)
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
    pickers: &mut color_picker::States,
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
        let browse = controls::button(ui, "...", ButtonKind::Neutral, browse_width, true);
        title_tooltip(ui, "Browse...");
        if browse {
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
        // The picker itself greys out and stops taking input when the colour is wired, as
        // `readonly` does in the Svelte component; a disabled scope would grey the whole
        // section including its title.
        if color_picker::draw(
            ui,
            "bg-color",
            &mut rgba,
            width - 24.0,
            wired,
            pickers,
            theme::PANEL_BG,
        ) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "bgColor".into(),
                value: ParamValue::Vector(
                    rgba.iter().map(|component| *component as f64).collect(),
                ),
            });
        }
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
        let browse = controls::button(ui, "...", ButtonKind::Neutral, browse_width, true);
        title_tooltip(ui, "Browse...");
        if browse {
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
        let body_origin = ui.cursor_screen_position();
        let empty = body.is_empty();
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
        if empty {
            // Dear ImGui's multiline field takes no hint, so the placeholder is drawn where
            // the first line would start.
            ui.draw_list().text_with_face(
                [
                    body_origin[0] + theme::INPUT_PADDING_X,
                    body_origin[1] + theme::INPUT_PADDING_X / 2.0,
                ],
                theme::TEXT.with_alpha(0.5),
                theme::face::VALUE,
                "Notes...",
            );
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
            let remove = controls::button(ui, "x", ButtonKind::Danger, 30.0, true);
            title_tooltip(ui, "Remove suffix");
            if remove {
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
            // `.empty-blocks` centres its line rather than starting it at the section's edge.
            let text = "Add blocks below to build a new filename";
            let origin = ui.cursor_screen_position();
            let content = width - 24.0;
            let size = controls::measure(ui, theme::face::SMALL_MONO, text);
            ui.dummy([content, 6.0]);
            ui.draw_list().text_with_face(
                [origin[0] + (content - size[0]) / 2.0, origin[1] + 6.0],
                theme::TEXT_BRIGHT.with_alpha(0.35),
                theme::face::SMALL_MONO,
                text,
            );
            ui.dummy([content, size[1] + 2.0]);
        }
        for index in 0..blocks.len() {
            let mut updated = blocks.clone();
            let change = rename_block_row(ui, width - 24.0, index, &mut updated[index]);
            if let Some(change) = change {
                if change == RowChange::Removed {
                    updated.remove(index);
                }
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "blocks".into(),
                    value: ParamValue::Structured(StructuredParam::RenameBlocks {
                        blocks: updated,
                    }),
                });
            }
            ui.dummy([width - 24.0, 1.0]);
        }

        ui.dummy([width - 24.0, 4.0]);
        // `.add-bar` gaps its three buttons by five, not the six the rest of the panel uses.
        let third = (width - 24.0 - 10.0) / 3.0;
        let additions: [(&str, RenameBlock, Color); 3] = [
            (
                "+ Text",
                RenameBlock::Text {
                    value: String::new(),
                },
                theme::ACCENT,
            ),
            (
                "+ Number",
                RenameBlock::Number {
                    start: 1.0,
                    pad: 3.0,
                },
                theme::COLOR_WARNING,
            ),
            (
                "+ Old Name",
                RenameBlock::Oldname {
                    find: String::new(),
                    replace_with: String::new(),
                },
                theme::COLOR_RENAME_ORIG,
            ),
        ];
        for (index, (label, block, tint)) in additions.into_iter().enumerate() {
            if add_button(ui, label, label, third, tint) {
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
                ui.same_line_at(0.0, 5.0);
            }
        }
    });

    let examples = rename_preview_names(context.image_names);
    let title = rename_preview_title(context.image_names.len());
    section(ui, width, &title, |ui| {
        let half = (width - 36.0) / 2.0;
        // `.preview-head` names the two columns in tracked upper case before the rows.
        let head = ui.cursor_screen_position();
        for (text, x) in [
            ("ORIGINAL", head[0]),
            ("NEW NAME", head[0] + half + 18.0),
        ] {
            controls::draw_tracked_text(
                ui,
                [x, head[1]],
                theme::TEXT_BRIGHT.with_alpha(0.4),
                theme::face::SMALL_MONO,
                text,
                0.06 * f32::from(theme::FONT_SIZE_XS),
            );
        }
        ui.dummy([width - 24.0, 17.0]);
        controls::row_separator(ui);
        ui.dummy([width - 24.0, 3.0]);
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
                    // `.preview-name--new.unchanged` drops back to plain dim text.
                    theme::TEXT_BRIGHT.with_alpha(0.35)
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
            // `.preview-example-note` is centred under the table, not ranged left.
            let note = "Example filenames shown above";
            let origin = ui.cursor_screen_position();
            let content = width - 24.0;
            let size = controls::measure(ui, theme::face::SMALL_MONO, note);
            ui.draw_list().text_with_face(
                [origin[0] + (content - size[0]) / 2.0, origin[1] + 4.0],
                theme::TEXT_BRIGHT.with_alpha(0.3),
                theme::face::SMALL_MONO,
                note,
            );
            ui.dummy([content, size[1] + 6.0]);
        }
    });
    edits
}

/// The heading over the rename preview, which counts files rather than `file(s)`.
pub fn rename_preview_title(images: usize) -> String {
    match images {
        0 => "Preview (no files loaded)".to_string(),
        1 => "Preview - 1 file".to_string(),
        count if count > 10 => format!("Preview - first 10 of {count} files"),
        count => format!("Preview - {count} files"),
    }
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

/// `.add-btn`: a short, subtle bar that takes its hover tint from the block it adds.
fn add_button(ui: &mut Ui, id: &str, label: &str, width: f32, tint: Color) -> bool {
    let height = 23.0;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##add-{id}"), [width, height]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
    }
    let max = [origin[0] + width, origin[1] + height];
    let list = ui.draw_list();
    list.rect(
        origin,
        max,
        if hovered {
            tint.mix(15.0, Color::TRANSPARENT)
        } else {
            theme::BTN_SUBTLE_BG
        },
        3.0,
        Rounding::All,
    );
    list.rect_outline(
        origin,
        max,
        if hovered { tint } else { theme::INPUT_BORDER },
        3.0,
        Rounding::All,
        1.0,
    );
    let size = list.measure(theme::face::SMALL_MONO, label);
    list.text_with_face(
        [
            origin[0] + (width - size[0]) / 2.0,
            origin[1] + (height - size[1]) / 2.0,
        ],
        if hovered { theme::TEXT } else { theme::TEXT_BRIGHT },
        theme::face::SMALL_MONO,
        label,
    );
    clicked
}

/// The compact field the rename blocks use: `.field-input`, which is smaller and lighter
/// than the thirty-pixel `.text-input` the rest of the inspector is built from.
fn compact_input(ui: &mut Ui, id: &str, value: &mut String, hint: &str, width: f32) -> bool {
    let origin = ui.cursor_screen_position();
    let max = [origin[0] + width, origin[1] + COMPACT_INPUT_HEIGHT];
    ui.draw_list()
        .rect(origin, max, theme::INPUT_BG, 3.0, Rounding::All);
    ui.set_next_item_width(width);
    let changed = ui.with_style(
        &[
            bite_imgui::StyleVar::FrameRounding(3.0),
            bite_imgui::StyleVar::FrameBorderSize(0.0),
            bite_imgui::StyleVar::FramePadding([
                7.0,
                (COMPACT_INPUT_HEIGHT
                    - controls::measure(ui, theme::face::SMALL_MONO, "Ag")[1])
                    / 2.0,
            ]),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::FrameBg, Color::TRANSPARENT),
                    (StyleColor::FrameBgHovered, Color::TRANSPARENT),
                    (StyleColor::FrameBgActive, Color::TRANSPARENT),
                    (StyleColor::Text, theme::TEXT_BRIGHT),
                    (StyleColor::TextDisabled, theme::TEXT.with_alpha(0.5)),
                ],
                |ui| {
                    ui.with_face(theme::face::SMALL_MONO, |ui| {
                        ui.input_text_with(
                            &format!("##{id}"),
                            value,
                            hint,
                            bite_imgui::InputFlags::default(),
                        )
                    })
                },
            )
        },
    );
    let active = ui.item_active();
    ui.draw_list().rect_outline(
        origin,
        max,
        if active {
            theme::ACCENT
        } else {
            theme::INPUT_BORDER
        },
        3.0,
        Rounding::All,
        1.0,
    );
    changed
}

/// `.block-badge`: a filled, tracked, bold tag rather than the outlined `.wired-badge`.
fn block_badge(ui: &mut Ui, text: &str, color: Color) -> f32 {
    let face = bite_imgui::Face::mono_weight(theme::FONT_SIZE_XS, bite_imgui::Weight::Bold);
    let tracking = 0.06 * f32::from(theme::FONT_SIZE_XS);
    let size = controls::measure_tracked(ui, face, text, tracking);
    let width = size[0] + 10.0;
    let origin = ui.cursor_screen_position();
    let top = origin[1] + (COMPACT_INPUT_HEIGHT - (size[1] + 4.0)) / 2.0;
    ui.draw_list().rect(
        [origin[0], top],
        [origin[0] + width, top + size[1] + 4.0],
        color.mix(18.0, Color::TRANSPARENT),
        2.0,
        Rounding::All,
    );
    controls::draw_tracked_text(ui, [origin[0] + 5.0, top + 2.0], color, face, text, tracking);
    width
}

/// The six-dot grab handle at the head of a reorderable row.
fn drag_handle(ui: &mut Ui, hovered: bool) {
    let origin = ui.cursor_screen_position();
    let color = theme::TEXT_BRIGHT.with_alpha(if hovered { 0.7 } else { 0.3 });
    let left = origin[0] + 2.0;
    let top = origin[1] + (COMPACT_INPUT_HEIGHT - 10.0) / 2.0;
    let list = ui.draw_list();
    for row in 0..3 {
        for column in 0..2 {
            list.circle(
                [
                    left + 1.5 + column as f32 * 3.0,
                    top + 1.5 + row as f32 * 3.5,
                ],
                1.2,
                color,
            );
        }
    }
}

/// One editable rename block. Returns the change it made, if any.
fn rename_block_row(
    ui: &mut Ui,
    width: f32,
    index: usize,
    block: &mut RenameBlock,
) -> Option<RowChange> {
    let mut change = None;
    let (label, colour) = match block {
        RenameBlock::Text { .. } => ("TEXT", theme::ACCENT),
        RenameBlock::Number { .. } => ("NUM", theme::COLOR_WARNING),
        RenameBlock::Oldname { .. } => ("ORIG", theme::COLOR_RENAME_ORIG),
    };
    let _id = ui.push_id(&format!("rename-{index}"));
    let row_origin = ui.cursor_screen_position();
    let row_height = COMPACT_INPUT_HEIGHT + 8.0;
    // `.block-row:hover` lifts the whole row, which is drawn before its contents.
    let hovered = ui.mouse_position()[0] >= row_origin[0]
        && ui.mouse_position()[0] <= row_origin[0] + width
        && ui.mouse_position()[1] >= row_origin[1]
        && ui.mouse_position()[1] <= row_origin[1] + row_height;
    if hovered {
        ui.draw_list().rect(
            row_origin,
            [row_origin[0] + width, row_origin[1] + row_height],
            theme::ITEM_HOVER_BG,
            3.0,
            Rounding::All,
        );
    }
    ui.set_cursor_screen_position([row_origin[0] + 2.0, row_origin[1] + 4.0]);

    ui.group(|ui| {
        let handle_width = 10.0;
        drag_handle(ui, hovered);
        ui.dummy([handle_width, COMPACT_INPUT_HEIGHT]);
        title_tooltip(ui, "Drag to reorder");
        ui.same_line_at(0.0, 5.0);

        let badge_width = block_badge(ui, label, colour);
        ui.dummy([badge_width, COMPACT_INPUT_HEIGHT]);
        ui.same_line_at(0.0, 5.0);

        // The handle, the badge, the delete button and their four gaps.
        let field_width = width - handle_width - badge_width - DELETE_WIDTH - 22.0;
        match block {
            RenameBlock::Text { value } => {
                if compact_input(ui, "text", value, "text...", field_width) {
                    change = Some(RowChange::Edited);
                }
            }
            RenameBlock::Number { start, pad } => {
                let sub = |ui: &mut Ui, text: &str| {
                    let origin = ui.cursor_screen_position();
                    let size = controls::measure(ui, theme::face::SMALL_MONO, text);
                    controls::draw_in_row(
                        ui,
                        origin[0],
                        origin[1],
                        COMPACT_INPUT_HEIGHT,
                        theme::TEXT_BRIGHT.with_alpha(0.5),
                        theme::face::SMALL_MONO,
                        text,
                    );
                    ui.dummy([size[0], COMPACT_INPUT_HEIGHT]);
                    ui.same_line_at(0.0, 4.0);
                };
                sub(ui, "start");
                let mut start_text = (*start as i64).to_string();
                if compact_input(ui, "start", &mut start_text, "", 46.0) {
                    if let Ok(parsed) = start_text.trim().parse::<f64>() {
                        if parsed >= 0.0 {
                            *start = parsed;
                            change = Some(RowChange::Edited);
                        }
                    }
                }
                ui.same_line_at(0.0, 4.0);
                sub(ui, "pad");
                let mut pad_text = (*pad as i64).to_string();
                if compact_input(ui, "pad", &mut pad_text, "", 46.0) {
                    if let Ok(parsed) = pad_text.trim().parse::<f64>() {
                        // The Svelte editor clamps padding to one through eight.
                        if (1.0..=8.0).contains(&parsed) {
                            *pad = parsed;
                            change = Some(RowChange::Edited);
                        }
                    }
                }
                ui.same_line_at(0.0, 7.0);
                // `.num-preview` shows what the first name would count from.
                let preview = format!("{:0>width$}", *start as i64, width = (*pad as usize).max(1));
                let origin = ui.cursor_screen_position();
                controls::draw_in_row(
                    ui,
                    origin[0],
                    origin[1],
                    COMPACT_INPUT_HEIGHT,
                    theme::COLOR_WARNING.with_alpha(0.85),
                    theme::face::SMALL_MONO,
                    &preview,
                );
                ui.dummy([
                    controls::measure(ui, theme::face::SMALL_MONO, &preview)[0],
                    COMPACT_INPUT_HEIGHT,
                ]);
            }
            RenameBlock::Oldname { find, replace_with } => {
                let half = (field_width - 22.0) / 2.0;
                if compact_input(ui, "find", find, "find...", half) {
                    change = Some(RowChange::Edited);
                }
                if !find.is_empty() {
                    ui.same_line_at(0.0, 4.0);
                    let origin = ui.cursor_screen_position();
                    controls::draw_in_row(
                        ui,
                        origin[0],
                        origin[1],
                        COMPACT_INPUT_HEIGHT,
                        theme::COLOR_RENAME_ORIG.with_alpha(0.6),
                        theme::face::SMALL_MONO,
                        "->",
                    );
                    ui.dummy([
                        controls::measure(ui, theme::face::SMALL_MONO, "->")[0],
                        COMPACT_INPUT_HEIGHT,
                    ]);
                    ui.same_line_at(0.0, 4.0);
                    if compact_input(ui, "replace", replace_with, "replace...", half) {
                        change = Some(RowChange::Edited);
                    }
                }
            }
        }

        // `.block-delete` sits hard right whatever the block is made of.
        ui.set_cursor_screen_position([
            row_origin[0] + width - DELETE_WIDTH,
            row_origin[1] + 4.0,
        ]);
        if delete_cross(ui, "delete", DELETE_WIDTH, COMPACT_INPUT_HEIGHT) {
            change = Some(RowChange::Removed);
        }
        title_tooltip(ui, "Remove block");
    });
    ui.set_cursor_screen_position([row_origin[0], row_origin[1] + row_height]);
    change
}

/// What a rename row asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RowChange {
    Edited,
    Removed,
}

/// `.block-delete`: a bare cross that reddens on hover.
fn delete_cross(ui: &mut Ui, id: &str, width: f32, height: f32) -> bool {
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##{id}"), [width, height]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
    }
    let colour = if hovered {
        theme::COLOR_ERROR
    } else {
        theme::TEXT_BRIGHT.with_alpha(0.3)
    };
    let centre = [origin[0] + width / 2.0, origin[1] + height / 2.0];
    let arm = 4.0;
    let list = ui.draw_list();
    list.line(
        [centre[0] - arm, centre[1] - arm],
        [centre[0] + arm, centre[1] + arm],
        colour,
        1.5,
    );
    list.line(
        [centre[0] - arm, centre[1] + arm],
        [centre[0] + arm, centre[1] - arm],
        colour,
        1.5,
    );
    clicked
}

/// One `.param-row`: a label above its control, closed by the row separator.
fn param_row(ui: &mut Ui, width: f32, label: &str, body: impl FnOnce(&mut Ui, f32)) {
    let padding = theme::INSPECTOR_PARAM_PADDING;
    ui.dummy([width, padding[1]]);
    let origin = ui.cursor_screen_position();
    ui.set_cursor_screen_position([origin[0] + padding[0], origin[1]]);
    controls::row_label(ui, label);
    ui.dummy([width, theme::INSPECTOR_PARAM_GAP]);
    ui.set_cursor_screen_position([origin[0] + padding[0], ui.cursor_screen_position()[1]]);
    body(ui, width - padding[0] * 2.0);
    ui.dummy([width, padding[1]]);
    controls::row_separator(ui);
}

/// A `.param-inline` row: the label on the left, a checkbox hard right.
fn inline_checkbox_row(ui: &mut Ui, width: f32, id: &str, label: &str, value: &mut bool) -> bool {
    let padding = theme::INSPECTOR_PARAM_PADDING;
    ui.dummy([width, padding[1]]);
    let origin = ui.cursor_screen_position();
    ui.set_cursor_screen_position([origin[0] + padding[0], origin[1]]);
    let changed = checkbox_row(ui, id, label, width - padding[0] * 2.0, value);
    ui.dummy([width, padding[1]]);
    controls::row_separator(ui);
    changed
}

/// A `.value-row`: a number box with its unit beside it. The dimension the aspect ratio
/// decides is shown but not editable, at the stylesheet's disabled opacity.
fn unit_row(
    ui: &mut Ui,
    width: f32,
    id: &str,
    value: &str,
    unit: &str,
    editable: bool,
) -> Option<String> {
    let unit_width = 24.0;
    let gap = 6.0;
    let field = (width - unit_width - gap).max(1.0);
    let origin = ui.cursor_screen_position();
    let mut text = value.to_string();
    let changed = controls::text_input_with(
        ui,
        id,
        &mut text,
        "",
        field,
        bite_imgui::InputFlags {
            read_only: !editable,
            ..bite_imgui::InputFlags::default()
        },
    );
    if !editable {
        // `.num-input:disabled` fades the whole box, which a read-only field does not.
        ui.draw_list().rect(
            origin,
            [origin[0] + field, origin[1] + theme::INPUT_HEIGHT],
            theme::PANEL_BG.with_alpha(1.0 - theme::DISABLED_OPACITY),
            theme::INPUT_RADIUS,
            Rounding::All,
        );
    }
    controls::draw_in_row(
        ui,
        origin[0] + field + gap,
        origin[1],
        theme::INPUT_HEIGHT,
        theme::TEXT_BRIGHT.with_alpha(0.4),
        theme::face::SMALL_MONO,
        unit,
    );
    ui.set_cursor_screen_position([origin[0], origin[1] + theme::INPUT_HEIGHT]);
    if changed { Some(text) } else { None }
}

/// The dimensions a resize would produce, as `previewW` and `previewH` work them out.
pub fn resize_preview(
    source: [u32; 2],
    relative: bool,
    preserve: bool,
    anchor: &str,
    width_value: f64,
    height_value: f64,
) -> Option<[i64; 2]> {
    let (source_width, source_height) = (f64::from(source[0]), f64::from(source[1]));
    if source_width <= 0.0 || source_height <= 0.0 {
        return None;
    }
    let (width, height) = if relative {
        (
            (source_width * width_value / 100.0).round(),
            if preserve {
                (source_height * width_value / 100.0).round()
            } else {
                (source_height * height_value / 100.0).round()
            },
        )
    } else if preserve {
        if anchor == "height" {
            (
                (source_width * height_value / source_height).round(),
                height_value,
            )
        } else {
            (
                width_value,
                (source_height * width_value / source_width).round(),
            )
        }
    } else {
        (width_value, height_value)
    };
    Some([width as i64, height as i64])
}

/// The Resize inspector, from `InspectorResizeNode.svelte`.
///
/// Its rows are not the definition's: the two dimensions share a unit, the one the aspect
/// ratio decides is shown rather than edited, and the section at the end says what the
/// selected image would come out as.
fn resize_node(ui: &mut Ui, width: f32, node: &GraphNode, context: &InspectorContext) -> Vec<Edit> {
    let mut edits = Vec::new();

    let mode = string_param(node, "mode", "absolute");
    let relative = mode == "relative";
    // Anything but an explicit false keeps the aspect ratio.
    let preserve = !matches!(
        node.data.params.get("preserve_aspect"),
        Some(ParamValue::Bool(false))
    );
    let anchor = string_param(node, "anchor", "width");
    let unit = if relative { "%" } else { "px" };
    let source = context.selected_image.unwrap_or([0, 0]);
    let has_source = source[0] > 0 && source[1] > 0;
    let ratio = if has_source {
        f64::from(source[0]) / f64::from(source[1])
    } else {
        1.0
    };

    let width_value = if relative {
        number_param(node, "scale_width", number_param(node, "scale", 100.0)).max(1.0)
    } else {
        number_param(node, "width", 1024.0).round().max(1.0)
    };
    let height_value = if relative {
        number_param(node, "scale_height", number_param(node, "scale", 100.0)).max(1.0)
    } else {
        number_param(node, "height", 1024.0).round().max(1.0)
    };
    let preview = resize_preview(source, relative, preserve, &anchor, width_value, height_value);

    let mut mode_index = usize::from(relative);
    param_row(ui, width, "Mode", |ui, content| {
        let labels: Vec<String> = vec!["Absolute".into(), "Relative".into()];
        if controls::dropdown(ui, "resize-mode", &mut mode_index, &labels, content, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "mode".into(),
                value: ParamValue::String(
                    ["absolute", "relative"][mode_index].into(),
                ),
            });
        }
    });

    let mut keep = preserve;
    if inline_checkbox_row(ui, width, "preserve-aspect", "Preserve Aspect Ratio", &mut keep) {
        edits.push(Edit::SetParam {
            node: node.id.clone(),
            name: "preserve_aspect".into(),
            value: ParamValue::Bool(keep),
        });
        // Turning it back on squares the other dimension up straight away.
        if keep {
            if relative {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "scale_height".into(),
                    value: ParamValue::Number(width_value),
                });
            } else if has_source {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "height".into(),
                    value: ParamValue::Int(((width_value / ratio).round() as i64).max(1)),
                });
            }
        }
    }

    if preserve && !relative {
        let mut anchor_index = usize::from(anchor == "height");
        param_row(ui, width, "Anchor", |ui, content| {
            let labels: Vec<String> = vec!["Width".into(), "Height".into()];
            if controls::dropdown(ui, "resize-anchor", &mut anchor_index, &labels, content, true) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: "anchor".into(),
                    value: ParamValue::String(["width", "height"][anchor_index].into()),
                });
            }
        });
    }

    // The dimension the anchor drives is computed, so it is shown rather than edited.
    let width_driven = preserve && !relative && anchor == "height";
    let height_driven = preserve && !relative && anchor == "width";
    let shown = |driven: bool, index: usize, fallback: f64| match (driven, preview) {
        (true, Some(size)) => size[index] as f64,
        _ => fallback,
    };

    let mut typed: Option<(bool, f64)> = None;
    param_row(ui, width, "Width", |ui, content| {
        let value = generic::trim_number(shown(width_driven, 0, width_value) as f32);
        if let Some(text) = unit_row(ui, content, "resize-width", &value, unit, !width_driven) {
            if let Ok(parsed) = text.trim().parse::<f64>() {
                typed = Some((true, parsed.max(1.0)));
            }
        }
    });
    param_row(ui, width, "Height", |ui, content| {
        let value = generic::trim_number(shown(height_driven, 1, height_value) as f32);
        if let Some(text) = unit_row(ui, content, "resize-height", &value, unit, !height_driven) {
            if let Ok(parsed) = text.trim().parse::<f64>() {
                typed = Some((false, parsed.max(1.0)));
            }
        }
    });
    if let Some((is_width, raw)) = typed {
        let value = if relative { raw } else { raw.round() };
        let number = |value: f64| {
            if relative {
                ParamValue::Number(value)
            } else {
                ParamValue::Int(value as i64)
            }
        };
        let mut set = |name: &str, value: ParamValue| {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: name.into(),
                value,
            });
        };
        match (relative, is_width) {
            (true, true) => {
                set("scale_width", number(value));
                if preserve {
                    set("scale_height", number(value));
                }
            }
            (true, false) => {
                set("scale_height", number(value));
                if preserve {
                    set("scale_width", number(value));
                }
            }
            (false, true) => {
                set("width", number(value));
                if preserve && has_source {
                    set("height", number((value / ratio).round().max(1.0)));
                }
            }
            (false, false) => {
                set("height", number(value));
                if preserve && has_source {
                    set("width", number((value * ratio).round().max(1.0)));
                }
            }
        }
    }

    let mut density_text: Option<String> = None;
    param_row(ui, width, "Resolution", |ui, content| {
        let density = number_param(node, "density", 72.0).round().clamp(1.0, 9600.0);
        let value = format!("{}", density as i64);
        density_text = unit_row(ui, content, "resize-density", &value, "dpi", true);
    });
    if let Some(text) = density_text {
        let parsed = text.trim().parse::<f64>().unwrap_or(72.0);
        edits.push(Edit::SetParam {
            node: node.id.clone(),
            name: "density".into(),
            value: ParamValue::Int((parsed.round() as i64).clamp(1, 9600)),
        });
    }

    const FILTERS: [&str; 4] = ["Lanczos", "Mitchell", "Catrom", "Point"];
    let current = string_param(node, "filter", "Lanczos");
    let mut filter_index = FILTERS
        .iter()
        .position(|option| *option == current)
        .unwrap_or(0);
    param_row(ui, width, "Filter", |ui, content| {
        let labels: Vec<String> = FILTERS.iter().map(|name| (*name).into()).collect();
        if controls::dropdown(ui, "resize-filter", &mut filter_index, &labels, content, true) {
            edits.push(Edit::SetParam {
                node: node.id.clone(),
                name: "filter".into(),
                value: ParamValue::String(FILTERS[filter_index].into()),
            });
        }
    });

    if let Some(size) = preview {
        section(ui, width, "Resize Preview", |ui| {
            for (label, value) in [
                ("Original", format!("{} x {}", source[0], source[1])),
                ("Resized", format!("{} x {}", size[0], size[1])),
            ] {
                let origin = ui.cursor_screen_position();
                let row = 18.0;
                controls::draw_in_row(
                    ui,
                    origin[0],
                    origin[1],
                    row,
                    theme::TEXT_BRIGHT.with_alpha(0.5),
                    theme::face::LABEL,
                    label,
                );
                let right = origin[0] + width - 24.0;
                controls::draw_in_row(
                    ui,
                    controls::right_aligned(ui, right, theme::face::VALUE, &value),
                    origin[1],
                    row,
                    theme::TEXT_BRIGHT,
                    theme::face::VALUE,
                    &value,
                );
                ui.dummy([width - 24.0, row + 6.0]);
            }
        });
    }
    edits
}

/// The Convert Format inspector: the format choice then that format's own parameters.
fn format_convert_node(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
    pickers: &mut color_picker::States,
) -> Vec<Edit> {
    let mut edits = generic::draw(ui, width, node, context, pickers);
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
        edits.extend(generic::row(ui, width, node, parameter, false, context, pickers));
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

fn number_param(node: &GraphNode, name: &str, fallback: f64) -> f64 {
    match node.data.params.get(name) {
        Some(ParamValue::Number(value)) => *value,
        Some(ParamValue::Int(value)) => *value as f64,
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
    fn the_rename_preview_counts_files_rather_than_file_s() {
        assert_eq!(rename_preview_title(0), "Preview (no files loaded)");
        assert_eq!(rename_preview_title(1), "Preview - 1 file");
        assert_eq!(rename_preview_title(4), "Preview - 4 files");
        assert_eq!(rename_preview_title(26), "Preview - first 10 of 26 files");
    }

    #[test]
    fn a_relative_resize_scales_both_sides_by_the_same_percentage_when_locked() {
        // 2048 x 1024 at fifty per cent, aspect kept: the height follows the width.
        let size = resize_preview([2048, 1024], true, true, "width", 50.0, 200.0).unwrap();
        assert_eq!(size, [1024, 512]);
        // Unlocked, each side takes its own percentage.
        let free = resize_preview([2048, 1024], true, false, "width", 50.0, 200.0).unwrap();
        assert_eq!(free, [1024, 2048]);
    }

    #[test]
    fn the_anchor_decides_which_absolute_dimension_is_computed() {
        // Anchored on the width, the height is worked out from the source ratio.
        let by_width = resize_preview([2000, 1000], false, true, "width", 800.0, 77.0).unwrap();
        assert_eq!(by_width, [800, 400]);
        // Anchored on the height, the width is.
        let by_height = resize_preview([2000, 1000], false, true, "height", 77.0, 400.0).unwrap();
        assert_eq!(by_height, [800, 400]);
        // With the ratio unlocked both are taken as typed.
        let free = resize_preview([2000, 1000], false, false, "width", 640.0, 480.0).unwrap();
        assert_eq!(free, [640, 480]);
    }

    #[test]
    fn nothing_is_previewed_until_an_image_is_selected() {
        assert_eq!(
            resize_preview([0, 0], false, true, "width", 800.0, 600.0),
            None
        );
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
