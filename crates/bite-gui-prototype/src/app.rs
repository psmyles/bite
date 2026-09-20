//! The application: window, frame loop, and the wiring between the panels and the model.
use crate::{
    canvas::{
        state::{PendingWire, WireEnd},
        view::{Action, Canvas, CanvasContext},
    },
    create_menu, dialogs, menu, modals, panels, persist, shell, studio, theme, work,
};
use bite_imgui::{Context, Key, MouseCursor, Vec2};
use bite_schema::{BuiltinNodeKind, NodeKind, ParamValue, Position};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

/// The images and preview state belonging to one Input branch.
#[derive(Default)]
pub struct Branch {
    pub paths: Vec<PathBuf>,
    pub thumbnails: Vec<panels::filmstrip::Thumbnail>,
    pub selected: Option<usize>,
    pub preview: Option<panels::preview::PreviewImage>,
    pub scroll: f32,
}

/// Everything the editor holds while it runs.
pub struct Editor {
    pub studio: studio::Studio,
    pub canvas: Canvas,
    pub library: panels::library::LibraryState,
    pub inspector: panels::inspector::InspectorState,
    pub create_menu: create_menu::CreateMenu,
    pub session: persist::Session,
    pub splitters: shell::SplitterState,

    /// The Input node whose images the filmstrip and preview are showing.
    pub active_input: Option<String>,
    pub branches: BTreeMap<String, Branch>,
    pub runtime_paths: BTreeMap<String, String>,
    pub resolved: BTreeMap<String, BTreeMap<String, ParamValue>>,

    pub selected_node: Option<String>,
    pub preview_node: Option<String>,
    pub show_preview_info: bool,

    pub modal: modals::Modal,
    pub progress: modals::Progress,
    pub status: String,
    pub timers_enabled: bool,

    pub run: Option<ActiveRun>,
    pub pending_action: Option<modals::PendingAction>,
    pub pending_open: Option<PathBuf>,
    pub jobs: work::Jobs,
    pub should_exit: bool,
    pub fullscreen: bool,
    /// Texture identifiers currently uploaded, so they can be released.
    pub textures: Vec<u64>,
    pub next_texture: u64,
    /// Decoded images waiting to become textures, keyed by their cache name.
    pub pending_uploads: Vec<(String, work::DecodedImage)>,
}

/// A workflow run in flight.
pub struct ActiveRun {
    pub cancelled: Arc<AtomicBool>,
    pub started: Instant,
    pub node: Option<String>,
    pub file: Option<String>,
}

/// A stable texture identifier for a cache key.
pub fn texture_id(key: &str) -> u64 {
    // A Fowler, Noll and Vo hash keeps identifiers stable across frames.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Zero and one are reserved for the font atlas.
    hash | 2
}

/// Where the node and format definitions live.
pub fn definitions_root() -> PathBuf {
    if let Some(explicit) = std::env::var_os("BITE_DEFINITIONS_ROOT") {
        return PathBuf::from(explicit);
    }
    // A packaged build keeps them beside the executable; a development build falls back
    // to the source tree.
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            if directory.join("node-definitions-v2").is_dir() {
                return directory.to_path_buf();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

impl Editor {
    pub fn new() -> Result<Self, String> {
        let root = definitions_root();
        let registry = studio::Studio::load_registry(&root)?;
        let studio = studio::Studio::seeded(registry);
        let session = persist::Session::load();
        Ok(Self {
            studio,
            canvas: Canvas::default(),
            library: panels::library::LibraryState::default(),
            inspector: panels::inspector::InspectorState::with_default_formats(),
            create_menu: create_menu::CreateMenu::default(),
            session,
            splitters: shell::SplitterState::default(),
            active_input: None,
            branches: BTreeMap::new(),
            runtime_paths: BTreeMap::new(),
            resolved: BTreeMap::new(),
            selected_node: None,
            preview_node: None,
            show_preview_info: true,
            modal: modals::Modal::None,
            progress: modals::Progress::default(),
            status: String::new(),
            timers_enabled: false,
            run: None,
            pending_action: None,
            pending_open: None,
            jobs: work::Jobs::default(),
            should_exit: false,
            fullscreen: false,
            textures: Vec::new(),
            next_texture: 2,
            pending_uploads: Vec::new(),
        })
    }

    /// The window title, with an asterisk while the workflow has unsaved edits.
    pub fn title(&self) -> String {
        let name = self
            .studio
            .path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled");
        let marker = if self.studio.dirty { "*" } else { "" };
        format!("{marker}{name} - Bite")
    }

    /// The images of the branch the filmstrip is showing.
    pub fn active_branch(&self) -> Option<&Branch> {
        self.active_input
            .as_ref()
            .and_then(|id| self.branches.get(id))
    }

    fn active_branch_mut(&mut self) -> Option<&mut Branch> {
        let id = self.active_input.clone()?;
        Some(self.branches.entry(id).or_default())
    }

    /// The file names of the active branch, for the inspector previews.
    pub fn active_names(&self) -> Vec<String> {
        self.active_branch()
            .map(|branch| {
                branch
                    .paths
                    .iter()
                    .map(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// How many images each Input branch holds, for the card footers.
    pub fn image_counts(&self) -> BTreeMap<String, usize> {
        self.branches
            .iter()
            .map(|(id, branch)| (id.clone(), branch.paths.len()))
            .collect()
    }

    /// Chooses the Input branch to show, following the selected node when it has one.
    pub fn sync_active_input(&mut self) {
        let inputs: Vec<String> = self
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .map(|node| node.id.clone())
            .collect();
        if let Some(selected) = &self.selected_node {
            if inputs.contains(selected) {
                self.active_input = Some(selected.clone());
                return;
            }
            if let Some(upstream) = self.upstream_input(selected) {
                self.active_input = Some(upstream);
                return;
            }
        }
        if self
            .active_input
            .as_ref()
            .is_none_or(|id| !inputs.contains(id))
        {
            self.active_input = inputs.first().cloned();
        }
    }

    /// Walks back to the Input node that feeds `node_id`, if there is one.
    fn upstream_input(&self, node_id: &str) -> Option<String> {
        let graph = &self.studio.workflow.graph;
        let mut pending = vec![node_id.to_string()];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(current) = pending.pop() {
            for edge in graph.edges.iter().filter(|edge| edge.target == current) {
                if !visited.insert(edge.source.clone()) {
                    continue;
                }
                if let Some(node) = graph.nodes.iter().find(|node| node.id == edge.source) {
                    if node.kind == NodeKind::Builtin(BuiltinNodeKind::Input) {
                        return Some(node.id.clone());
                    }
                    pending.push(node.id.clone());
                }
            }
        }
        None
    }

    /// Applies one canvas action to the model.
    pub fn apply_canvas_action(&mut self, action: Action) {
        match action {
            Action::BeginDrag => self.studio.begin_transaction(),
            Action::MoveNodes(moves) => {
                let graph = &mut self.studio.workflow.graph;
                // A child of a group stores a position relative to its parent.
                let parents: BTreeMap<String, [f64; 2]> = graph
                    .nodes
                    .iter()
                    .map(|node| (node.id.clone(), [node.position.x, node.position.y]))
                    .collect();
                for (id, position) in moves {
                    let parent_offset = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == id)
                        .and_then(|node| node.parent_id.clone())
                        .and_then(|parent| parents.get(&parent).copied())
                        .unwrap_or([0.0, 0.0]);
                    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
                        node.position = Position {
                            x: f64::from(position[0]) - parent_offset[0],
                            y: f64::from(position[1]) - parent_offset[1],
                        };
                    }
                }
                self.studio.dirty = true;
            }
            Action::EndDrag => self.studio.finish_transaction("Moved nodes"),
            Action::Connect {
                source,
                source_handle,
                target,
                target_handle,
            } => {
                match self
                    .studio
                    .connect(&source, &source_handle, &target, &target_handle)
                {
                    Ok(_) => self.status.clear(),
                    Err(error) => self.status = format!("Connection rejected: {error}"),
                }
                self.request_preview();
            }
            Action::DeleteEdges(ids) => {
                for id in ids {
                    self.studio.delete_edge(&id);
                }
                self.request_preview();
            }
            Action::OpenCreateMenu { position, pending } => {
                self.create_menu.open_at(position, pending);
                self.create_menu.can_group = self.can_group();
                self.create_menu.can_ungroup = self.can_ungroup();
            }
            Action::TogglePreview(id) => {
                self.preview_node = if self.preview_node.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    Some(id)
                };
                self.request_preview();
            }
            Action::ToggleBypass(id) => {
                let enabled = self
                    .studio
                    .workflow
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .map(|node| {
                        !matches!(
                            node.data.params.get("_enabled"),
                            Some(ParamValue::Bool(false)) | Some(ParamValue::Int(0))
                        )
                    })
                    .unwrap_or(true);
                self.studio
                    .set_param(&id, "_enabled".into(), ParamValue::Bool(!enabled));
                self.request_preview();
            }
            Action::ResizeNode { id, width, height } => {
                if let Some(node) = self
                    .studio
                    .workflow
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == id)
                {
                    node.width = Some(f64::from(width));
                    node.height = Some(f64::from(height));
                }
                self.studio.dirty = true;
            }
            Action::SetParam { node, name, value } => {
                self.studio.set_param(&node, name, value);
            }
            Action::DropDefinition { payload, position } => {
                self.create_node(&payload, position, None);
            }
            Action::SelectionChanged => {
                self.selected_node = self.canvas.state.selected_nodes.iter().next().cloned();
                self.sync_active_input();
                self.request_preview();
            }
        }
    }

    /// True when the selection can be grouped: at least one node that is not an endpoint.
    pub fn can_group(&self) -> bool {
        self.canvas.state.selected_nodes.iter().any(|id| {
            self.studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == *id)
                .is_some_and(|node| {
                    !matches!(
                        node.kind,
                        NodeKind::Builtin(
                            BuiltinNodeKind::Input
                                | BuiltinNodeKind::ImageOutput
                                | BuiltinNodeKind::TextOutput
                                | BuiltinNodeKind::FlipbookOutput
                                | BuiltinNodeKind::Group
                        )
                    )
                })
        })
    }

    pub fn can_ungroup(&self) -> bool {
        self.canvas.state.selected_nodes.iter().any(|id| {
            self.studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == *id)
                .is_some_and(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Group))
        })
    }

    /// Creates a node from a library or menu entry, connecting a dropped wire in one step.
    pub fn create_node(
        &mut self,
        payload: &str,
        position: Vec2,
        pending: Option<PendingWire>,
    ) {
        // The card's corner lands under the pointer. Centring would need the height before
        // the node exists, and guessing it puts the card somewhere the pointer never was.
        let place = Position {
            x: f64::from(position[0]),
            y: f64::from(position[1]),
        };

        self.studio.begin_transaction();
        let created = match payload {
            "inputNode" => Some(self.studio.add_input(place)),
            "imageOutputNode" => Some(self.studio.add_output(place)),
            "textOutputNode" => Some(self.studio.add_text_output(place)),
            "flipbookOutputNode" => Some(self.studio.add_flipbook_output(place)),
            "commentNode" => Some(self.studio.add_comment(place)),
            definition => self.studio.add_processing(definition, place).ok(),
        };
        let Some(created) = created else {
            self.studio.finish_transaction("Create failed");
            self.status = format!("Unknown node type: {payload}");
            return;
        };

        if let Some(pending) = pending {
            let entry = self.entry_for(payload);
            let side = if pending.end == WireEnd::Source {
                WireEnd::Target
            } else {
                WireEnd::Source
            };
            match entry.and_then(|entry| {
                create_menu::first_matching_handle(
                    &entry,
                    pending.wire,
                    side,
                    &self.studio.registry,
                )
            }) {
                Some(handle) => {
                    let result = if pending.end == WireEnd::Source {
                        self.studio
                            .connect(&pending.node, &pending.handle, &created, &handle)
                    } else {
                        self.studio
                            .connect(&created, &handle, &pending.node, &pending.handle)
                    };
                    if let Err(error) = result {
                        self.status = format!("Created node; connection rejected: {error}");
                    }
                }
                None => self.status = "New node has no compatible port".into(),
            }
        }
        self.studio.finish_transaction("Created node");
        self.canvas.state.select_only([created.clone()]);
        self.selected_node = Some(created);
        self.sync_active_input();
        self.request_preview();
    }

    fn entry_for(&self, payload: &str) -> Option<panels::library::Entry> {
        panels::library::workflow_entries()
            .into_iter()
            .chain(panels::library::definition_entries(&self.studio.registry))
            .find(|entry| entry.id == payload)
    }

    /// Marks the preview as needing a refresh on the next idle moment.
    pub fn request_preview(&mut self) {
        self.jobs.request_preview();
    }

    /// The node the preview should follow. A workflow endpoint or a comment is never a
    /// preview target, so the selection only counts when it can produce an image.
    /// The node the preview renders through.
    ///
    /// Only a double click sets this, as in Electron. Selecting a node must not retarget
    /// the preview: a node that produces no image would leave the panel empty and move the
    /// PREVIEWING badge onto a card that is merely selected.
    pub fn effective_preview_node(&self) -> Option<String> {
        self.preview_node.clone()
    }
}

/// Runs the editor.
pub fn run(initial: Option<PathBuf>) -> Result<(), String> {
    crate::platform::run(initial)
}

/// Builds the canvas context for one frame.
pub fn canvas_context<'a>(
    editor: &'a Editor,
    counts: &'a BTreeMap<String, usize>,
    delta: f32,
) -> CanvasContext<'a> {
    CanvasContext {
        registry: &editor.studio.registry,
        resolved: &editor.resolved,
        image_counts: counts,
        preview_node: editor.effective_preview_node(),
        running_node: editor.run.as_ref().and_then(|run| run.node.clone()),
        running_file: editor.run.as_ref().and_then(|run| run.file.clone()),
        delta,
    }
}

/// Draws one frame of the whole interface.
pub fn draw_frame(
    editor: &mut Editor,
    context: &mut Context,
    size: Vec2,
    scale: f32,
    delta: f32,
) -> bite_imgui::DrawData {
    let mut frame = context.frame(size[0], size[1], scale, delta);
    let mut ui = frame.ui();

    // The shell paints the gap color behind every panel.
    ui.background_draw_list().rect(
        [0.0, 0.0],
        size,
        theme::GAP_COLOR,
        0.0,
        bite_imgui::Rounding::None,
    );

    let command = ui.with_face(theme::face::BODY, |ui| {
        menu::draw(
            ui,
            &editor.title(),
            editor.timers_enabled,
            cfg!(debug_assertions),
        )
    });

    let layout = shell::compute(size, menu::MENU_BAR_HEIGHT, editor.session.panels);
    let counts = editor.image_counts();

    // Panels.
    if let Some(entry) = panels::library::draw(
        &mut ui,
        layout.library,
        &mut editor.library,
        &editor.studio.registry,
        delta,
    ) {
        let centre = editor
            .canvas
            .state
            .viewport
            .to_graph([layout.canvas.width() / 2.0, layout.canvas.height() / 2.0]);
        editor.create_node(&entry.id, centre, None);
    }

    // The canvas is moved out for the draw so that it can borrow the model immutably,
    // then put back before its actions are applied.
    let graph = editor.studio.workflow.graph.clone();
    let mut canvas = std::mem::take(&mut editor.canvas);
    let actions = {
        let context = canvas_context(editor, &counts, delta);
        canvas.run(&mut ui, layout.canvas, &graph, &context)
    };
    editor.canvas = canvas;
    for action in actions {
        editor.apply_canvas_action(action);
    }

    draw_inspector(editor, &mut ui, layout.inspector, delta);
    let preview = editor
        .active_branch()
        .and_then(|branch| branch.preview.clone());
    if panels::preview::draw(
        &mut ui,
        layout.preview,
        preview.as_ref(),
        editor.show_preview_info,
    ) {
        editor.show_preview_info = !editor.show_preview_info;
    }

    let thumbnails = editor
        .active_branch()
        .map(|branch| branch.thumbnails.clone())
        .unwrap_or_default();
    let selected = editor.active_branch().and_then(|branch| branch.selected);
    let mut scroll = editor
        .active_branch()
        .map(|branch| branch.scroll)
        .unwrap_or(0.0);
    if let Some(outcome) =
        panels::filmstrip::draw(&mut ui, layout.filmstrip, &thumbnails, selected, &mut scroll)
    {
        handle_filmstrip(editor, outcome);
    }
    if let Some(branch) = editor.active_branch_mut() {
        branch.scroll = scroll;
    }

    // The splitters sit above the panels so their gaps stay clickable.
    let shell_height = size[1] - menu::MENU_BAR_HEIGHT;
    let mut panels_sizes = editor.session.panels;
    if editor
        .splitters
        .run(&mut ui, &layout, &mut panels_sizes, shell_height)
    {
        editor.session.panels = panels_sizes;
    }

    // The creation menu draws above the canvas.
    let anchor = {
        let local = editor.canvas.state.viewport.to_screen(editor.create_menu.position);
        [
            layout.canvas.min[0] + local[0],
            layout.canvas.min[1] + local[1],
        ]
    };
    if let Some(outcome) =
        editor
            .create_menu
            .draw(&mut ui, anchor, size, &editor.studio.registry, delta)
    {
        handle_create_menu(editor, outcome);
    }

    if let Some(outcome) = modals::draw(&mut ui, &editor.modal, &editor.progress, size) {
        handle_modal(editor, outcome);
    }

    if let Some(command) = command {
        crate::commands::run(editor, command);
    }
    handle_shortcuts(editor, &ui);

    // The interface handle borrows the frame, so it is released before rendering.
    let _ = ui;
    frame.render()
}

fn draw_inspector(editor: &mut Editor, ui: &mut bite_imgui::Ui, rect: shell::Rect, delta: f32) {
    let names = editor.active_names();
    let graph = editor.studio.workflow.graph.clone();
    let selected = editor
        .selected_node
        .as_ref()
        .and_then(|id| graph.nodes.iter().find(|node| node.id == *id))
        .cloned();
    let candidates = crate::commands::run_candidates(editor);
    let ready = candidates.iter().filter(|node| node.valid()).count();
    let context = panels::inspector::InspectorContext {
        registry: &editor.studio.registry,
        graph: &graph,
        resolved: editor
            .selected_node
            .as_ref()
            .and_then(|id| editor.resolved.get(id)),
        image_names: &names,
        runtime_paths: &editor.runtime_paths,
        running: editor.run.is_some(),
        run_ready: ready > 0,
        run_tooltip: crate::commands::run_tooltip(&candidates),
        delta,
    };
    let edits = panels::inspector::draw(ui, rect, selected.as_ref(), &mut editor.inspector, &context);
    for edit in edits {
        crate::commands::apply_edit(editor, edit);
    }
}

fn handle_filmstrip(editor: &mut Editor, outcome: panels::filmstrip::Outcome) {
    match outcome {
        panels::filmstrip::Outcome::Selected(index) => {
            if let Some(branch) = editor.active_branch_mut() {
                branch.selected = Some(index);
            }
            editor.request_preview();
        }
        panels::filmstrip::Outcome::RequestImport => {
            if let Some(node) = editor.active_input.clone() {
                crate::commands::add_individual_images(editor, &node);
            }
        }
        panels::filmstrip::Outcome::Dropped(paths) => {
            if let Some(node) = editor.active_input.clone() {
                crate::commands::add_paths(editor, &node, paths.iter().map(PathBuf::from).collect());
            }
        }
    }
}

fn handle_create_menu(editor: &mut Editor, outcome: create_menu::Outcome) {
    match outcome {
        create_menu::Outcome::Create(entry) => {
            let position = editor.create_menu.position;
            let pending = editor.create_menu.pending.clone();
            editor.create_node(&entry.id, position, pending);
        }
        create_menu::Outcome::GroupSelection => crate::commands::group_selection(editor),
        create_menu::Outcome::UngroupSelection => crate::commands::ungroup_selection(editor),
        create_menu::Outcome::Dismissed => {}
    }
}

fn handle_modal(editor: &mut Editor, outcome: modals::Outcome) {
    match outcome {
        modals::Outcome::Dismissed => {
            editor.modal = modals::Modal::None;
            editor.pending_action = None;
        }
        modals::Outcome::ConfirmDiscard(action) | modals::Outcome::ConfirmSave(action) => {
            editor.modal = modals::Modal::None;
            crate::commands::perform_pending(editor, action);
        }
        modals::Outcome::RunSelected => {
            editor.modal = modals::Modal::None;
            crate::commands::start_run(editor);
        }
        modals::Outcome::CancelBatch => {
            if let Some(run) = &editor.run {
                run.cancelled
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            editor.status = "Cancelling...".into();
        }
        modals::Outcome::CancelImport => {
            editor.jobs.cancel_import();
            editor.modal = modals::Modal::None;
        }
        modals::Outcome::OpenOutputFolder(path) => {
            let _ = dialogs::open_path(Path::new(&path));
        }
        modals::Outcome::OpenUrl(url) => {
            let _ = dialogs::open_url(&url);
        }
    }
}

/// Keyboard commands that are not on the menu, plus the canvas editing shortcuts.
fn handle_shortcuts(editor: &mut Editor, ui: &bite_imgui::Ui) {
    // A focused text field owns every key, so no shortcut may fire.
    if ui.key_pressed(Key::Escape) && !matches!(editor.modal, modals::Modal::None) {
        // A modal that offers a cancel closes on Escape.
        if !matches!(editor.modal, modals::Modal::BatchProgress) {
            editor.modal = modals::Modal::None;
            editor.pending_action = None;
        }
    }
    if !matches!(editor.modal, modals::Modal::None) || editor.create_menu.open {
        return;
    }

    let primary = ui.primary_modifier();
    let shift = ui.shift_down();
    if ui.key_pressed(Key::Delete) || ui.key_pressed(Key::Backspace) {
        crate::commands::delete_selection(editor);
    }
    if primary && shift && ui.key_pressed(Key::G) {
        crate::commands::ungroup_selection(editor);
    } else if primary && ui.key_pressed(Key::G) {
        crate::commands::group_selection(editor);
    }
    if !primary && (ui.key_pressed(Key::Space) || ui.key_pressed(Key::Tab)) {
        let position = editor.canvas.state.last_pointer;
        editor.create_menu.open_at(position, None);
        editor.create_menu.can_group = editor.can_group();
        editor.create_menu.can_ungroup = editor.can_ungroup();
    }
    let map = [
        (Key::N, "n"),
        (Key::O, "o"),
        (Key::S, "s"),
        (Key::R, "r"),
        (Key::Z, "z"),
        (Key::Y, "y"),
        (Key::X, "x"),
        (Key::C, "c"),
        (Key::V, "v"),
        (Key::D, "d"),
        (Key::A, "a"),
        (Key::Digit0, "0"),
        (Key::Minus, "-"),
        (Key::Equal, "="),
    ];
    for (key, name) in map {
        if ui.key_pressed(key) {
            if let Some(command) = menu::shortcut(
                name,
                menu::Modifiers {
                    primary,
                    shift,
                },
            ) {
                crate::commands::run(editor, command);
            }
        }
    }
    if ui.key_pressed(Key::F11) {
        crate::commands::run(editor, menu::Command::ToggleFullScreen);
    }
}

/// Applies the cursor the interface asked for to the window.
pub fn cursor_for(cursor: MouseCursor) -> winit::window::CursorIcon {
    use winit::window::CursorIcon;
    match cursor {
        MouseCursor::Arrow => CursorIcon::Default,
        MouseCursor::TextInput => CursorIcon::Text,
        MouseCursor::ResizeAll => CursorIcon::Move,
        MouseCursor::ResizeNorthSouth => CursorIcon::RowResize,
        MouseCursor::ResizeEastWest => CursorIcon::ColResize,
        MouseCursor::ResizeNorthEastSouthWest => CursorIcon::NeswResize,
        MouseCursor::ResizeNorthWestSouthEast => CursorIcon::NwseResize,
        MouseCursor::Hand => CursorIcon::Pointer,
        MouseCursor::NotAllowed => CursorIcon::NotAllowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_identifiers_are_stable_and_avoid_the_font_slots() {
        let first = texture_id("thumbnail:a:0");
        assert_eq!(first, texture_id("thumbnail:a:0"));
        assert_ne!(first, texture_id("thumbnail:a:1"));
        assert!(first >= 2);
    }

    #[test]
    fn the_title_marks_unsaved_edits_and_names_the_file() {
        let registry = bite_core::Registry::default();
        let mut editor = Editor {
            studio: studio::Studio::seeded(registry),
            canvas: Canvas::default(),
            library: Default::default(),
            inspector: Default::default(),
            create_menu: Default::default(),
            session: Default::default(),
            splitters: Default::default(),
            active_input: None,
            branches: Default::default(),
            runtime_paths: Default::default(),
            resolved: Default::default(),
            selected_node: None,
            preview_node: None,
            show_preview_info: true,
            modal: modals::Modal::None,
            progress: Default::default(),
            status: String::new(),
            timers_enabled: false,
            run: None,
            pending_action: None,
            pending_open: None,
            jobs: Default::default(),
            should_exit: false,
            fullscreen: false,
            textures: Vec::new(),
            next_texture: 2,
            pending_uploads: Vec::new(),
        };
        editor.studio.dirty = false;
        assert_eq!(editor.title(), "Untitled - Bite");
        editor.studio.dirty = true;
        assert_eq!(editor.title(), "*Untitled - Bite");
        editor.studio.path = Some(PathBuf::from("/tmp/my-workflow.bite"));
        assert_eq!(editor.title(), "*my-workflow - Bite");
    }
}
