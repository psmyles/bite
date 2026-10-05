//! The application: window, frame loop, and the wiring between the panels and the model.
use crate::{
    canvas::{
        state::{PendingWire, WireEnd},
        view::{node_enabled, Action, Canvas, CanvasContext},
    },
    create_menu, dialogs, menu, modals, panels, persist, shell, studio, theme, work,
};
use bite_imgui::{Context, Key, MouseCursor, Vec2};
use bite_schema::{BuiltinNodeKind, NodeKind, ParamValue, Position};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

/// The images and preview state belonging to one Input branch.
#[derive(Default)]
pub struct Branch {
    pub paths: Vec<PathBuf>,
    pub thumbnails: Vec<panels::filmstrip::Thumbnail>,
    pub selected: Option<usize>,
    pub preview: Option<panels::preview::PreviewImage>,
    /// The file the last preview was made from. The render runs on a thumbnail, so the
    /// overlay takes its measurements from here rather than from the rendered image.
    pub preview_source: work::SourceImage,
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
    /// The Debug menu's log window, which reads the session log file.
    pub log_window: crate::log_window::LogWindow,
    /// The Debug menu's interface showcase.
    pub showcase: crate::showcase::Showcase,
    pub progress: modals::Progress,
    /// When the running import began, so the dialog can report how long it took. It is
    /// cleared once the import ends, which freezes the time the completion line shows.
    pub import_started: Option<std::time::Instant>,
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
    // The top bit is Dear ImGui's half of the identifier space - since 1.92 it creates textures
    // of its own, the font atlas among them, and both sides land in the one map the renderer
    // keeps. Clearing the bit here and setting it there keeps the two disjoint by construction
    // rather than by the unlikelihood of a hash collision. Zero and one stay out of the way of
    // the "no texture" identifier.
    (hash & !bite_imgui::IMGUI_TEXTURE_ID_BIT) | 2
}

/// Where the node and format definitions live.
pub fn definitions_root() -> PathBuf {
    if let Some(explicit) = std::env::var_os("BITE_DEFINITIONS_ROOT") {
        return PathBuf::from(explicit);
    }
    // A packaged build keeps them beside the executable - or, in a macOS bundle, one level up in
    // `Contents/Resources`, since `Contents/MacOS` holds only executables. A development build
    // falls back to the source tree.
    //
    // The order matters on neither platform (only one of the two can exist) but the `Resources`
    // probe is what makes a bundle launched from Finder work at all: its working directory is
    // `/`, so nothing relative is any help, and without this the fallback below points at a
    // source tree that is not on the user's machine. `bite-cli`'s `load_registry` has the same
    // list for the same reason.
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            if directory.join("node-definitions").is_dir() {
                return directory.to_path_buf();
            }
            let resources = directory.join("../Resources");
            if resources.join("node-definitions").is_dir() {
                return resources;
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
            log_window: Default::default(),
            showcase: Default::default(),
            progress: modals::Progress::default(),
            import_started: None,
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

    /// The filmstrip entry the active branch has selected, if any.
    pub fn selected_thumbnail(&self) -> Option<&panels::filmstrip::Thumbnail> {
        let branch = self.active_branch()?;
        branch.thumbnails.get(branch.selected?)
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
    pub fn create_node(&mut self, payload: &str, position: Vec2, pending: Option<PendingWire>) {
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

    /// Stops the import clock, leaving the completion line reporting the time it took.
    pub fn finish_import_timing(&mut self) {
        if let Some(started) = self.import_started.take() {
            self.progress.elapsed_seconds = started.elapsed().as_secs_f32();
        }
    }

    /// Marks the preview as needing a refresh on the next idle moment.
    pub fn request_preview(&mut self) {
        self.jobs.request_preview();
    }

    /// Resolves the value nodes with no image to render through.
    ///
    /// A Compare, an Add or a Float still shows its number on the canvas before anything
    /// is imported, which is what the Electron preview did with nothing selected. Nothing
    /// here reads a file, so it costs one pass over the graph.
    pub fn resolve_values(&mut self) {
        let values = bite_core::preview::values(&self.studio.workflow.graph, &self.studio.registry)
            .unwrap_or_default();
        self.resolved = crate::work::resolved_params(values);
    }

    /// Puts the selected thumbnail in the preview panel while nothing has been rendered
    /// there yet, so an imported image appears at once rather than after the first render.
    /// `Preview.svelte` seeds the same stand-in, and leaves a rendered image in place.
    pub fn seed_preview_from_thumbnail(&mut self) {
        if self
            .active_branch()
            .is_some_and(|branch| branch.preview.is_some())
        {
            return;
        }
        self.show_thumbnail_as_preview();
    }

    /// Puts the selected thumbnail in the preview panel, replacing whatever is there.
    ///
    /// This is what the panel shows when no chain processes the image: Electron's pipeline
    /// returns the same downscaled file when the preview graph has no nodes in it.
    pub fn show_thumbnail_as_preview(&mut self) {
        let Some(branch) = self.active_branch_mut() else {
            return;
        };
        let Some(thumbnail) = branch
            .selected
            .and_then(|index| branch.thumbnails.get(index))
            .filter(|thumbnail| thumbnail.texture.is_some())
        else {
            return;
        };
        branch.preview = Some(panels::preview::PreviewImage {
            texture: thumbnail.texture,
            width: thumbnail.source.width.max(1),
            height: thumbnail.source.height.max(1),
            name: thumbnail.name.clone(),
            format: thumbnail
                .path
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_string())
                .unwrap_or_default(),
            bytes: thumbnail.source.bytes,
            pixels: thumbnail.size,
        });
    }

    /// The node the preview renders through.
    ///
    /// Only a double click sets the choice, as in Electron. Selecting a node must not
    /// retarget the preview: a node that produces no image would leave the panel empty and
    /// move the PREVIEWING badge onto a card that is merely selected. With no choice made,
    /// the last processing node in the chain stands in, so a freshly opened workflow
    /// previews without being asked to.
    pub fn effective_preview_node(&self) -> Option<String> {
        let graph = &self.studio.workflow.graph;
        let registry = &self.studio.registry;
        let chosen = self.preview_node.as_ref().and_then(|id| {
            let node = graph.nodes.iter().find(|node| node.id == *id)?;
            let previewable = matches!(node.kind, NodeKind::Processing(_))
                && crate::canvas::view::node_enabled(node, graph, registry);
            previewable.then(|| id.clone())
        });
        chosen.or_else(|| self.auto_preview_node())
    }

    /// The node the preview falls back to: the end of the chain the outputs are fed from,
    /// or the last processing node reachable from an Input when nothing is wired to an
    /// output. `Preview.svelte` picks the same node and paints its badge from it.
    fn auto_preview_node(&self) -> Option<String> {
        let graph = &self.studio.workflow.graph;
        let registry = &self.studio.registry;
        let is_output = |id: &str| {
            graph.nodes.iter().any(|node| {
                node.id == id
                    && matches!(
                        node.kind,
                        NodeKind::Builtin(
                            BuiltinNodeKind::ImageOutput
                                | BuiltinNodeKind::TextOutput
                                | BuiltinNodeKind::FlipbookOutput
                        )
                    )
            })
        };

        // Everything an Input can reach, following edges forward.
        let mut reachable: Vec<String> = graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .map(|node| node.id.clone())
            .collect();
        let mut index = 0;
        while index < reachable.len() {
            let id = reachable[index].clone();
            index += 1;
            for edge in &graph.edges {
                if edge.source == id && !reachable.contains(&edge.target) {
                    reachable.push(edge.target.clone());
                }
            }
        }

        let enabled = |id: &str| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .is_some_and(|node| node_enabled(node, graph, registry))
        };
        let processing = |id: &str| {
            graph
                .nodes
                .iter()
                .any(|node| node.id == id && matches!(node.kind, NodeKind::Processing(_)))
        };

        // What an output is fed from, when anything is.
        let feeding = graph.edges.iter().find(|edge| {
            is_output(&edge.target)
                && edge.target_handle.starts_with("in:")
                && reachable.contains(&edge.source)
                && processing(&edge.source)
        });
        if let Some(edge) = feeding {
            if enabled(&edge.source) {
                return Some(edge.source.clone());
            }
            if let Some(earlier) = self.non_bypassed_predecessor(&edge.source) {
                return Some(earlier);
            }
        }

        // Otherwise the last processing node that nothing else processes after.
        let candidates: Vec<&String> = reachable
            .iter()
            .filter(|id| processing(id) && enabled(id))
            .collect();
        let terminal = candidates.iter().rev().find(|id| {
            !graph.edges.iter().any(|edge| {
                edge.source == ***id && reachable.contains(&edge.target) && !is_output(&edge.target)
            })
        });
        terminal
            .or_else(|| candidates.last())
            .map(|id| (*id).clone())
    }

    /// Walks back from a bypassed node to the nearest active one feeding its image input.
    fn non_bypassed_predecessor(&self, from: &str) -> Option<String> {
        let graph = &self.studio.workflow.graph;
        let registry = &self.studio.registry;
        let mut current = from.to_string();
        // The chain is finite, and the guard stops a cycle from spinning here.
        for _ in 0..graph.nodes.len() {
            let edge = graph
                .edges
                .iter()
                .find(|edge| edge.target == current && edge.target_handle.starts_with("in:"))?;
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .filter(|node| matches!(node.kind, NodeKind::Processing(_)))?;
            if node_enabled(node, graph, registry) {
                return Some(node.id.clone());
            }
            current = node.id.clone();
        }
        None
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
        menu_wire: editor
            .create_menu
            .open
            .then(|| {
                editor
                    .create_menu
                    .pending
                    .clone()
                    .map(|pending| (pending, editor.create_menu.position))
            })
            .flatten(),
        delta,
    }
}

/// Draws one frame of the whole interface.
///
/// Nothing comes back: the frame is handed to sokol_imgui, which ends it and draws it into
/// whichever sokol_gfx pass the caller has open. So this must be called with a pass about to be
/// opened, and `render::imgui::render` called inside it.
pub fn draw_frame(editor: &mut Editor, context: &mut Context, size: Vec2, scale: f32, delta: f32) {
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

    let bar = ui.with_face(theme::face::BODY, |ui| {
        menu::draw(
            ui,
            &editor.title(),
            editor.timers_enabled,
            cfg!(debug_assertions),
        )
    });
    let command = bar.command;

    let layout = shell::compute(size, bar.height, editor.session.panels);
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
    if let Some(outcome) = panels::filmstrip::draw(
        &mut ui,
        layout.filmstrip,
        &thumbnails,
        selected,
        &mut scroll,
    ) {
        handle_filmstrip(editor, outcome);
    }
    if let Some(branch) = editor.active_branch_mut() {
        branch.scroll = scroll;
    }

    // The splitters sit above the panels so their gaps stay clickable.
    let shell_height = size[1] - bar.height;
    let mut panels_sizes = editor.session.panels;
    if editor
        .splitters
        .run(&mut ui, &layout, &mut panels_sizes, shell_height)
    {
        editor.session.panels = panels_sizes;
    }

    // The creation menu draws above the canvas.
    let anchor = {
        let local = editor
            .canvas
            .state
            .viewport
            .to_screen(editor.create_menu.position);
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

    // The debug windows sit above the panels and below the modals, which are exclusive.
    if crate::log_window::draw(&mut ui, &mut editor.log_window) {
        if let Some(path) = crate::logging::log_path() {
            let _ = dialogs::open_path(&path);
        }
    }

    if let Some(crate::showcase::Outcome::Show(modal)) =
        crate::showcase::draw(&mut ui, &mut editor.showcase)
    {
        editor.modal = modal;
    }

    if let Some(outcome) = modals::draw(&mut ui, &editor.modal, &editor.progress, size) {
        handle_modal(editor, outcome);
    }

    if let Some(command) = command {
        crate::commands::run(editor, command);
    }
    handle_shortcuts(editor, &ui);

    // The interface handle borrows the frame, so it is released before it is handed over.
    let _ = ui;
    frame.submit();
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
        selected_image: editor
            .selected_thumbnail()
            .map(|thumbnail| [thumbnail.source.width, thumbnail.source.height]),
        runtime_paths: &editor.runtime_paths,
        running: editor.run.is_some(),
        run_ready: ready > 0,
        run_tooltip: crate::commands::run_tooltip(&candidates),
        delta,
    };
    let edits =
        panels::inspector::draw(ui, rect, selected.as_ref(), &mut editor.inspector, &context);
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
            editor.seed_preview_from_thumbnail();
            editor.request_preview();
        }
        panels::filmstrip::Outcome::RequestImport => {
            if let Some(node) = editor.active_input.clone() {
                crate::commands::add_individual_images(editor, &node);
            }
        }
        panels::filmstrip::Outcome::Dropped(paths) => {
            if let Some(node) = editor.active_input.clone() {
                crate::commands::add_paths(
                    editor,
                    &node,
                    paths.iter().map(PathBuf::from).collect(),
                );
            }
        }
    }
}

fn handle_create_menu(editor: &mut Editor, outcome: create_menu::Outcome) {
    match outcome {
        create_menu::Outcome::Create {
            entry,
            position,
            pending,
        } => editor.create_node(&entry.id, position, pending),
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
            editor.import_started = None;
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
            if let Some(command) = menu::shortcut(name, menu::Modifiers { primary, shift }) {
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
    fn texture_identifiers_are_stable_and_stay_out_of_imguis_half() {
        let first = texture_id("thumbnail:a:0");
        assert_eq!(first, texture_id("thumbnail:a:0"));
        assert_ne!(first, texture_id("thumbnail:a:1"));
        assert!(first >= 2);
        // Whatever the hash, the identifier must never carry Dear ImGui's bit: the font atlas
        // and a thumbnail share the renderer's one texture map.
        for key in [
            "thumbnail:a:0",
            "preview:node:3",
            "",
            "a very long key indeed",
        ] {
            assert_eq!(texture_id(key) & bite_imgui::IMGUI_TEXTURE_ID_BIT, 0);
        }
    }

    /// An editor over the seeded workflow, for the checks that only read the graph.
    fn bare_editor() -> Editor {
        let registry = bite_core::Registry::default();
        Editor {
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
            log_window: Default::default(),
            showcase: Default::default(),
            progress: Default::default(),
            import_started: None,
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
        }
    }

    /// A wire dropped on empty canvas opens the menu, and the node the menu creates is
    /// joined to the port the drag started from, in one step.
    #[test]
    fn a_node_created_from_a_dropped_wire_is_connected_to_it() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut editor = bare_editor();
        editor.studio = studio::Studio::seeded(studio::Studio::load_registry(&root).unwrap());
        let input = editor
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .unwrap()
            .id
            .clone();
        let dropped = PendingWire {
            node: input.clone(),
            handle: "out:output".into(),
            end: WireEnd::Source,
            origin: [0.0, 0.0],
            wire: bite_core::graph::WireType::Image,
        };
        editor.create_menu.open_at([320.0, 200.0], Some(dropped));
        let entry = editor.entry_for("grayscale").unwrap();
        let outcome = editor.create_menu.create(entry);
        // Choosing a row closes the menu before the outcome is acted on.
        editor.create_menu.close();
        handle_create_menu(&mut editor, outcome);

        let created = editor.selected_node.clone().unwrap();
        let edge = editor
            .studio
            .workflow
            .graph
            .edges
            .iter()
            .find(|edge| edge.target == created)
            .expect("the new node is wired to the port the drag came from");
        assert_eq!(edge.source, input);
        assert_eq!(edge.source_handle, "out:output");
        assert_eq!(edge.target_handle, "in:input");
        assert!(editor.status.is_empty(), "{}", editor.status);

        // Dragging the other way round, out of an input port, wires the new node into it.
        let output = editor
            .studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput))
            .unwrap()
            .id
            .clone();
        editor.create_menu.open_at(
            [480.0, 200.0],
            Some(PendingWire {
                node: output.clone(),
                handle: "in:input".into(),
                end: WireEnd::Target,
                origin: [0.0, 0.0],
                wire: bite_core::graph::WireType::Image,
            }),
        );
        let entry = editor.entry_for("blur").unwrap();
        let outcome = editor.create_menu.create(entry);
        editor.create_menu.close();
        handle_create_menu(&mut editor, outcome);
        let blur = editor.selected_node.clone().unwrap();
        let edge = editor
            .studio
            .workflow
            .graph
            .edges
            .iter()
            .find(|edge| edge.target == output)
            .expect("the new node feeds the port the drag came from");
        assert_eq!(edge.source, blur);
        assert_eq!(edge.source_handle, "out:output");
        assert!(editor.status.is_empty(), "{}", editor.status);
    }

    /// A number dragged out of a value node lands on the first parameter of the new node
    /// that can take one, not on an image port.
    #[test]
    fn a_dropped_value_wire_lands_on_a_parameter_port() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut editor = bare_editor();
        editor.studio = studio::Studio::seeded(studio::Studio::load_registry(&root).unwrap());
        let float = editor
            .studio
            .add_processing("value_float", Position { x: 0.0, y: 0.0 })
            .unwrap();
        editor.create_menu.open_at(
            [320.0, 320.0],
            Some(PendingWire {
                node: float.clone(),
                handle: "param:value".into(),
                end: WireEnd::Source,
                origin: [0.0, 0.0],
                wire: bite_core::graph::WireType::Number,
            }),
        );
        let entry = editor.entry_for("blur").unwrap();
        let outcome = editor.create_menu.create(entry);
        editor.create_menu.close();
        handle_create_menu(&mut editor, outcome);
        let blur = editor.selected_node.clone().unwrap();
        let edge = editor
            .studio
            .workflow
            .graph
            .edges
            .iter()
            .find(|edge| edge.target == blur)
            .expect("the number reaches a parameter of the new node");
        assert_eq!(edge.source, float);
        assert_eq!(edge.source_handle, "param:value");
        assert!(
            edge.target_handle.starts_with("param:"),
            "{}",
            edge.target_handle
        );
        assert!(editor.status.is_empty(), "{}", editor.status);
    }

    #[test]
    fn the_title_marks_unsaved_edits_and_names_the_file() {
        let mut editor = bare_editor();
        editor.studio.dirty = false;
        assert_eq!(editor.title(), "Untitled - Bite");
        editor.studio.dirty = true;
        assert_eq!(editor.title(), "*Untitled - Bite");
        editor.studio.path = Some(PathBuf::from("/tmp/my-workflow.bite"));
        assert_eq!(editor.title(), "*my-workflow - Bite");
    }
    /// Adds a processing node with no parameters, for the preview target checks.
    fn process_node(id: &str) -> bite_schema::GraphNode {
        bite_schema::GraphNode {
            id: id.into(),
            kind: NodeKind::Processing(bite_schema::ProcessingNodeKind::Process),
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: bite_schema::NodeData {
                label: id.into(),
                definition_id: "grayscale".into(),
                params: Default::default(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        }
    }

    fn image_edge(id: &str, source: &str, target: &str) -> bite_schema::GraphEdge {
        bite_schema::GraphEdge {
            id: id.into(),
            source: source.into(),
            source_handle: "out:output".into(),
            target: target.into(),
            target_handle: "in:input".into(),
        }
    }

    /// An Input, two processing nodes and an Image Output, wired in a line.
    fn chained_editor() -> Editor {
        let mut editor = bare_editor();
        let graph = &mut editor.studio.workflow.graph;
        let input = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .map(|node| node.id.clone())
            .expect("the seed has an Input");
        let output = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput))
            .map(|node| node.id.clone())
            .expect("the seed has an Image Output");
        graph.nodes.push(process_node("first"));
        graph.nodes.push(process_node("second"));
        graph.edges.push(image_edge("e1", &input, "first"));
        graph.edges.push(image_edge("e2", "first", "second"));
        graph.edges.push(image_edge("e3", "second", &output));
        editor
    }

    #[test]
    fn the_preview_follows_the_node_feeding_the_output_when_nothing_is_chosen() {
        let editor = chained_editor();
        assert_eq!(editor.effective_preview_node().as_deref(), Some("second"));
    }

    #[test]
    fn a_chosen_node_beats_the_one_feeding_the_output() {
        let mut editor = chained_editor();
        editor.preview_node = Some("first".into());
        assert_eq!(editor.effective_preview_node().as_deref(), Some("first"));
    }

    #[test]
    fn a_bypassed_node_hands_the_preview_back_to_the_one_before_it() {
        let mut editor = chained_editor();
        let graph = &mut editor.studio.workflow.graph;
        let node = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "second")
            .unwrap();
        node.data
            .params
            .insert("_enabled".into(), ParamValue::Bool(false));
        assert_eq!(editor.effective_preview_node().as_deref(), Some("first"));
    }

    #[test]
    fn with_nothing_wired_to_an_output_the_end_of_the_chain_previews() {
        let mut editor = chained_editor();
        editor
            .studio
            .workflow
            .graph
            .edges
            .retain(|edge| edge.id != "e3");
        assert_eq!(editor.effective_preview_node().as_deref(), Some("second"));
    }

    #[test]
    fn a_workflow_without_processing_previews_through_nothing() {
        // The panel then renders the selected image itself, as the Electron preview does.
        let editor = bare_editor();
        assert_eq!(editor.effective_preview_node(), None);
    }
}
