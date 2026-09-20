//! Menu commands, inspector edits and the run flow.
use crate::{
    app::{ActiveRun, Editor},
    dialogs, menu, modals,
    panels::inspector::Edit,
    studio, work,
};
use bite_imagemagick::{Magick, import::ThumbnailCache};
use bite_schema::{BuiltinNodeKind, GraphNode, NodeKind, ParamValue};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

/// Runs one menu command.
pub fn run(editor: &mut Editor, command: menu::Command) {
    use menu::Command as C;
    match command {
        C::New => guard(editor, modals::PendingAction::New),
        C::OpenWorkflow => guard(editor, modals::PendingAction::Open),
        C::SaveWorkflow => save(editor, false),
        C::SaveWorkflowAs => save(editor, true),
        C::RunWorkflow => request_run(editor),
        C::Exit => guard(editor, modals::PendingAction::Exit),
        C::ExportPowerShell => export(editor, bite_core::cli_export::Shell::PowerShell, "ps1"),
        C::ExportBash => export(editor, bite_core::cli_export::Shell::Bash, "sh"),
        C::ExportCmd => export(editor, bite_core::cli_export::Shell::Cmd, "bat"),
        C::Undo => {
            if editor.studio.undo() {
                editor.canvas.state.clear_selection();
                editor.selected_node = None;
                editor.request_preview();
            }
        }
        C::Redo => {
            if editor.studio.redo() {
                editor.canvas.state.clear_selection();
                editor.selected_node = None;
                editor.request_preview();
            }
        }
        C::Cut => {
            copy_selection(editor);
            delete_selection(editor);
        }
        C::Copy => copy_selection(editor),
        C::Paste => paste(editor),
        C::Duplicate => duplicate_selection(editor),
        C::Delete => delete_selection(editor),
        C::SelectAll => {
            let ids: Vec<String> = editor
                .studio
                .workflow
                .graph
                .nodes
                .iter()
                .map(|node| node.id.clone())
                .collect();
            editor.canvas.state.select_only(ids);
            editor.selected_node = editor.canvas.state.selected_nodes.iter().next().cloned();
        }
        C::ActualSize => {
            editor.canvas.state.viewport.zoom = 1.0;
        }
        C::ZoomIn => {
            let centre = [0.0, 0.0];
            editor
                .canvas
                .state
                .viewport
                .zoom_about(centre, crate::theme::ZOOM_STEP);
        }
        C::ZoomOut => {
            let centre = [0.0, 0.0];
            editor
                .canvas
                .state
                .viewport
                .zoom_about(centre, 1.0 / crate::theme::ZOOM_STEP);
        }
        C::ToggleFullScreen => editor.fullscreen = !editor.fullscreen,
        C::TogglePerformanceTimers => {
            editor.timers_enabled = !editor.timers_enabled;
            editor.status = if editor.timers_enabled {
                "Performance timers enabled".into()
            } else {
                "Performance timers disabled".into()
            };
        }
        C::ViewLog => {
            let path = crate::logging::log_path();
            match path {
                Some(path) => {
                    let _ = dialogs::open_path(&path);
                }
                None => editor.status = "No log file is available".into(),
            }
        }
        C::OpenTempFolder => {
            let directory = work::cache_directory();
            let _ = std::fs::create_dir_all(&directory);
            let _ = dialogs::open_path(&directory);
        }
        C::ClearCache => {
            match work::clear_cache() {
                Ok(count) => editor.status = format!("Cleared {count} cached file(s)"),
                Err(error) => editor.status = error,
            }
        }
        C::ShowAllUiElements => {
            editor.status = "The interface showcase opens from the Debug menu".into();
        }
        C::About => {
            editor.modal = modals::Modal::About {
                versions: about_versions(),
            };
        }
        C::Documentation => {
            let _ = dialogs::open_url(menu::DOCUMENTATION_URL);
        }
        C::ReportBug => {
            let _ = dialogs::open_url(menu::REPORT_BUG_URL);
        }
        C::Credits => editor.modal = modals::Modal::Credits,
        C::CheckForUpdates => {
            editor.modal = modals::Modal::Update(modals::UpdateState::Checking);
            let handle = editor.jobs.handle();
            std::thread::spawn(move || {
                handle.send(work::Message::UpdateChecked(Box::new(
                    crate::updates::check(),
                )));
            });
        }
    }
}

/// The dependency versions the About dialog lists.
pub fn about_versions() -> Vec<(String, String)> {
    let mut host = Magick::discover(Arc::new(AtomicBool::new(false)));
    let magick = bite_core::execution::ImageHost::capture(&mut host, &["-version".to_string()])
        .ok()
        .and_then(|text| parse_magick_version(&text))
        .unwrap_or_else(|| "unknown".into());
    vec![
        ("ImageMagick".into(), magick),
        ("Dear ImGui".into(), "1.90.9".into()),
        ("wgpu".into(), "30.0".into()),
        ("winit".into(), "0.30".into()),
        (
            "Rust".into(),
            option_env!("CARGO_PKG_RUST_VERSION")
                .unwrap_or("stable")
                .into(),
        ),
    ]
}

/// Reads the version out of the banner ImageMagick prints.
pub fn parse_magick_version(text: &str) -> Option<String> {
    let line = text.lines().next()?;
    let rest = line.strip_prefix("Version: ImageMagick ")?;
    Some(rest.split_whitespace().next()?.to_string())
}

/// Runs an action, prompting first when the workflow has unsaved edits.
fn guard(editor: &mut Editor, action: modals::PendingAction) {
    if editor.studio.dirty {
        editor.pending_action = Some(action);
        editor.modal = modals::Modal::Confirm {
            message: modals::confirm_message(action),
            pending: action,
        };
    } else {
        perform_pending(editor, action);
    }
}

/// Carries out an action the unsaved-changes prompt was guarding.
pub fn perform_pending(editor: &mut Editor, action: modals::PendingAction) {
    editor.pending_action = None;
    match action {
        modals::PendingAction::New => {
            let registry = editor.studio.registry.clone();
            editor.studio = studio::Studio::seeded(registry);
            editor.canvas.state.clear_selection();
            editor.canvas.state.viewport = Default::default();
            editor.selected_node = None;
            editor.preview_node = None;
            editor.branches.clear();
            editor.active_input = None;
            editor.sync_active_input();
        }
        modals::PendingAction::Open => {
            if let Ok(Some(path)) = dialogs::open_workflow() {
                open_path(editor, &path);
            }
        }
        modals::PendingAction::OpenPath => {
            if let Some(path) = editor.pending_open.take() {
                open_path(editor, &path);
            }
        }
        modals::PendingAction::Exit => editor.should_exit = true,
    }
}

/// Opens a workflow file, reporting a version mismatch the way Electron does.
pub fn open_path(editor: &mut Editor, path: &Path) {
    let registry = editor.studio.registry.clone();
    match studio::Studio::open(registry, path) {
        Ok(studio) => {
            editor.studio = studio;
            editor.canvas.state.clear_selection();
            editor.canvas.state.viewport = crate::canvas::viewport::Viewport {
                x: editor.studio.workflow.graph.viewport.x as f32,
                y: editor.studio.workflow.graph.viewport.y as f32,
                zoom: (editor.studio.workflow.graph.viewport.zoom as f32).clamp(
                    crate::theme::ZOOM_MIN,
                    crate::theme::ZOOM_MAX,
                ),
            };
            editor.selected_node = None;
            editor.preview_node = None;
            editor.branches.clear();
            editor.active_input = None;
            editor.sync_active_input();
            editor.status = format!("Opened {}", path.display());
        }
        Err(error) if error.contains("version") => {
            editor.modal = modals::Modal::IncompatibleVersion { file_version: None };
        }
        Err(error) => {
            editor.modal = modals::Modal::Message {
                title: "Bite".into(),
                body: format!("Failed to open workflow:\n{error}"),
            };
        }
    }
}

fn save(editor: &mut Editor, force_dialog: bool) {
    let target = if force_dialog || editor.studio.path.is_none() {
        match dialogs::save_file("Save Workflow", "Bite Workflow", "bite", "workflow.bite") {
            Ok(Some(path)) => Some(path),
            _ => None,
        }
    } else {
        editor.studio.path.clone()
    };
    let Some(target) = target else { return };
    // The viewport is stored with the graph, so it is captured before writing.
    editor.studio.workflow.graph.viewport = bite_schema::Viewport {
        x: f64::from(editor.canvas.state.viewport.x),
        y: f64::from(editor.canvas.state.viewport.y),
        zoom: f64::from(editor.canvas.state.viewport.zoom),
    };
    match editor.studio.save(&target) {
        Ok(()) => editor.status = format!("Saved {}", target.display()),
        Err(error) => {
            editor.modal = modals::Modal::Message {
                title: "Bite".into(),
                body: format!("Failed to save workflow:\n{error}"),
            };
        }
    }
}

fn export(editor: &mut Editor, shell: bite_core::cli_export::Shell, extension: &str) {
    let default = format!("bite-batch.{extension}");
    let Ok(Some(path)) = dialogs::save_file(
        "Export CLI Script",
        "Script",
        extension,
        &default,
    ) else {
        return;
    };
    match editor.studio.export_cli(&path, shell) {
        Ok(()) => editor.status = format!("Exported {}", path.display()),
        Err(error) => {
            editor.modal = modals::Modal::Message {
                title: "Bite".into(),
                body: format!("Failed to export CLI script:\n{error}"),
            };
        }
    }
}

// -- Graph editing -------------------------------------------------------------------

pub fn delete_selection(editor: &mut Editor) {
    let selected: Vec<String> = editor.canvas.state.selected_nodes.iter().cloned().collect();
    let edges: Vec<String> = editor.canvas.state.selected_edges.iter().cloned().collect();
    let mut changed = false;
    for edge in edges {
        changed |= editor.studio.delete_edge(&edge);
    }
    if !selected.is_empty() {
        changed |= editor.studio.delete_selection(&selected);
    }
    if changed {
        editor.canvas.state.clear_selection();
        editor.selected_node = None;
        editor.sync_active_input();
        editor.request_preview();
    }
}

pub fn copy_selection(editor: &mut Editor) {
    let selected: Vec<String> = editor.canvas.state.selected_nodes.iter().cloned().collect();
    if editor.studio.copy_selection(&selected) {
        if let Some(text) = editor.studio.clipboard_json() {
            let _ = dialogs::write_clipboard(&text);
        }
    }
}

pub fn paste(editor: &mut Editor) {
    if let Ok(Some(text)) = dialogs::read_clipboard() {
        editor.studio.import_clipboard_json(&text);
    }
    let created = editor.studio.paste();
    if !created.is_empty() {
        editor.canvas.state.select_only(created.clone());
        editor.selected_node = created.first().cloned();
        editor.request_preview();
    }
}

pub fn duplicate_selection(editor: &mut Editor) {
    let selected: Vec<String> = editor.canvas.state.selected_nodes.iter().cloned().collect();
    let created = editor.studio.duplicate_selection(&selected);
    if !created.is_empty() {
        editor.canvas.state.select_only(created.clone());
        editor.selected_node = created.first().cloned();
        editor.request_preview();
    }
}

pub fn group_selection(editor: &mut Editor) {
    let selected: Vec<String> = editor.canvas.state.selected_nodes.iter().cloned().collect();
    if let Some(group) = editor.studio.group_selection(&selected) {
        editor.canvas.state.select_only([group.clone()]);
        editor.selected_node = Some(group);
    }
}

pub fn ungroup_selection(editor: &mut Editor) {
    let selected: Vec<String> = editor.canvas.state.selected_nodes.iter().cloned().collect();
    if editor.studio.ungroup_selection(&selected) {
        editor.canvas.state.clear_selection();
        editor.selected_node = None;
    }
}

// -- Inspector edits -----------------------------------------------------------------

/// Applies one inspector edit.
pub fn apply_edit(editor: &mut Editor, edit: Edit) {
    match edit {
        Edit::SetParam { node, name, value } => {
            editor.studio.set_param(&node, name, value);
            editor.request_preview();
        }
        Edit::SetRuntimePath { node, path } => {
            editor.runtime_paths.insert(node, path);
        }
        Edit::ImportImages { node } => import_scanned(editor, &node),
        Edit::AddIndividualImages { node } => add_individual_images(editor, &node),
        Edit::ClearImages { node } => {
            editor.branches.remove(&node);
            editor.request_preview();
        }
        Edit::SelectImage { node, index } => {
            let branch = editor.branches.entry(node).or_default();
            branch.selected = Some(index);
            editor.request_preview();
        }
        Edit::RunWorkflow => request_run(editor),
        Edit::BrowseFolder { node, name } => {
            if let Ok(Some(path)) = dialogs::select_folder() {
                editor.studio.set_param(
                    &node,
                    name,
                    ParamValue::String(path.to_string_lossy().into_owned()),
                );
            }
        }
        Edit::BrowseFile {
            node,
            name,
            extension,
        } => {
            let default = format!("output.{extension}");
            if let Ok(Some(path)) = dialogs::save_file(
                "Choose output file",
                "Output",
                &extension,
                &default,
            ) {
                editor.studio.set_param(
                    &node,
                    name,
                    ParamValue::String(path.to_string_lossy().into_owned()),
                );
            }
        }
        Edit::SetScanRecursive { node, value } => {
            editor.inspector.scan_recursive = value;
            rescan(editor, &node);
        }
        Edit::ToggleScanFormat { node, group } => {
            if !editor.inspector.scan_formats.remove(&group) {
                editor.inspector.scan_formats.insert(group);
            }
            rescan(editor, &node);
        }
        Edit::SetInputFolder { node } => {
            if let Ok(Some(path)) = dialogs::select_folder() {
                editor.inspector.scan_folder = path.to_string_lossy().into_owned();
                rescan(editor, &node);
            }
        }
    }
}

/// Counts the files the current scan options would import.
fn rescan(editor: &mut Editor, node: &str) {
    if editor.inspector.scan_folder.is_empty() {
        editor.inspector.scan_count = None;
        return;
    }
    let root = PathBuf::from(&editor.inspector.scan_folder);
    let recursive = editor.inspector.scan_recursive;
    let extensions = active_extensions(editor);
    let handle = editor.jobs.handle();
    let node = node.to_string();
    editor.inspector.scanning = true;
    std::thread::spawn(move || {
        let cancelled = AtomicBool::new(false);
        let count = bite_imagemagick::import::scan_folder(&root, recursive, 8, &cancelled)
            .map(|paths| {
                paths
                    .iter()
                    .filter(|path| matches_extensions(path, &extensions))
                    .count()
            })
            .unwrap_or(0);
        handle.send(work::Message::ScanFinished { node, count });
    });
}

/// The file extensions the enabled format chips cover.
pub fn active_extensions(editor: &Editor) -> Vec<String> {
    crate::panels::inspector::scan_format_groups()
        .into_iter()
        .filter(|(name, _)| editor.inspector.scan_formats.contains(*name))
        .flat_map(|(_, extensions)| extensions.into_iter().map(str::to_string))
        .collect()
}

/// True when a path carries one of the enabled extensions.
pub fn matches_extensions(path: &Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| extensions.contains(&extension))
}

/// Imports every file the current scan options match.
fn import_scanned(editor: &mut Editor, node: &str) {
    if editor.inspector.scan_folder.is_empty() {
        return;
    }
    let root = PathBuf::from(&editor.inspector.scan_folder);
    let recursive = editor.inspector.scan_recursive;
    let extensions = active_extensions(editor);
    let cancelled = AtomicBool::new(false);
    let paths = bite_imagemagick::import::scan_folder(&root, recursive, 8, &cancelled)
        .map(|paths| {
            paths
                .into_iter()
                .filter(|path| matches_extensions(path, &extensions))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    add_paths(editor, node, paths);
}

/// Opens the image picker and imports whatever is chosen.
pub fn add_individual_images(editor: &mut Editor, node: &str) {
    if let Ok(paths) = dialogs::select_images() {
        if !paths.is_empty() {
            add_paths(editor, node, paths);
        }
    }
}

/// Imports a set of files into a branch, generating thumbnails off the interface thread.
pub fn add_paths(editor: &mut Editor, node: &str, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let size = thumbnail_size(editor, node);
    let handle = editor.jobs.handle();
    editor.jobs.reset_import_cancel();
    let cancelled = editor.jobs.import_cancelled.clone();
    let node_id = node.to_string();
    editor.modal = modals::Modal::ImportProgress;
    editor.progress = modals::Progress {
        completed: 0,
        total: paths.len(),
        elapsed_seconds: 0.0,
        error: None,
    };

    std::thread::spawn(move || {
        let mut magick = Magick::discover(cancelled);
        let mut cache = ThumbnailCache::new(work::cache_directory());
        let mut thumbnails = Vec::new();
        // Batching keeps the number of spawned processes low.
        for (index, chunk) in paths.chunks(16).enumerate() {
            match cache.load_batch(&mut magick, chunk, size) {
                Ok(infos) => {
                    for info in infos {
                        if let Ok(image) = decode_thumbnail(&mut magick, &info.thumbnail) {
                            thumbnails.push(work::Thumbnail {
                                path: info.path.clone(),
                                image,
                            });
                        }
                    }
                }
                Err(error) => {
                    handle.send(work::Message::ImportFailed {
                        node: node_id,
                        error,
                    });
                    return;
                }
            }
            handle.send(work::Message::ImportProgress {
                node: node_id.clone(),
                completed: ((index + 1) * 16).min(paths.len()),
                total: paths.len(),
            });
        }
        handle.send(work::Message::ImportFinished {
            node: node_id,
            paths,
            thumbnails,
        });
    });
}

/// Decodes a cached thumbnail, converting it through ImageMagick when it is not already a
/// portable network graphic. The cache stores compact formats, so most entries convert.
fn decode_thumbnail(magick: &mut Magick, path: &Path) -> Result<work::DecodedImage, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return work::decode_png(&bytes);
    }
    let converted = path.with_extension("decoded.png");
    bite_core::execution::ImageHost::run(
        magick,
        &[
            path.to_string_lossy().into_owned(),
            converted.to_string_lossy().into_owned(),
        ],
    )?;
    let decoded = std::fs::read(&converted).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&converted);
    work::decode_png(&decoded)
}

/// The thumbnail size configured on an Input node.
fn thumbnail_size(editor: &Editor, node: &str) -> u32 {
    editor
        .studio
        .workflow
        .graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == node)
        .and_then(|node| node.data.params.get("thumbnailSize"))
        .and_then(|value| match value {
            ParamValue::Int(value) => Some(*value as u32),
            ParamValue::Number(value) => Some(*value as u32),
            _ => None,
        })
        .unwrap_or(256)
        .clamp(64, 2048)
}

// -- Running -------------------------------------------------------------------------

/// The output nodes and why each can or cannot run, from `RunWorkflowButton.svelte`.
pub fn run_candidates(editor: &Editor) -> Vec<modals::RunCandidate> {
    let graph = &editor.studio.workflow.graph;
    graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                NodeKind::Builtin(
                    BuiltinNodeKind::ImageOutput
                        | BuiltinNodeKind::TextOutput
                        | BuiltinNodeKind::FlipbookOutput
                )
            )
        })
        .map(|node| modals::RunCandidate {
            id: node.id.clone(),
            label: crate::canvas::view::card_label(node, &editor.studio.registry),
            reasons: output_problems(editor, node),
        })
        .collect()
}

/// Every reason an output node cannot run, in the order the Svelte button reports them.
pub fn output_problems(editor: &Editor, node: &GraphNode) -> Vec<String> {
    let graph = &editor.studio.workflow.graph;
    let mut reasons = Vec::new();
    let connected = graph
        .edges
        .iter()
        .any(|edge| edge.target == node.id && edge.target_handle == "in:input");
    if !connected {
        reasons.push("No image input connected".to_string());
        return reasons;
    }
    let string_param = |name: &str| match node.data.params.get(name) {
        Some(ParamValue::String(text)) => text.clone(),
        _ => String::new(),
    };
    match node.kind {
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            if string_param("outputPath") == "custom" {
                let folder_edge = graph
                    .edges
                    .iter()
                    .find(|edge| edge.target == node.id && edge.target_handle == "in:folder");
                if let Some(edge) = folder_edge {
                    let empty = graph
                        .nodes
                        .iter()
                        .find(|candidate| candidate.id == edge.source)
                        .map(|source| match source.data.params.get("folderPath") {
                            Some(ParamValue::String(path)) => path.is_empty(),
                            _ => true,
                        })
                        .unwrap_or(true);
                    if empty {
                        reasons.push("Connected folder path node has no folder set".into());
                    }
                } else if string_param("customPath").is_empty() {
                    reasons.push("Custom output folder is empty".into());
                }
            }
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            if string_param("outputPath").is_empty() {
                reasons.push("Output file path is empty".into());
            }
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput)
            if string_param("flipbookOutputPath").is_empty() => {
                reasons.push("Output file path is empty".into());
            }
        _ => {}
    }
    // Only when nothing else is wrong does an empty branch become the reason.
    if reasons.is_empty() {
        let images = editor
            .branches
            .values()
            .map(|branch| branch.paths.len())
            .sum::<usize>();
        if images == 0 {
            reasons.push("No images loaded for connected Input node".into());
        }
    }
    reasons
}

/// The run button's tooltip, which reports how many outputs are ready.
pub fn run_tooltip(candidates: &[modals::RunCandidate]) -> String {
    if candidates.is_empty() {
        return "No output nodes in graph".into();
    }
    let valid = candidates.iter().filter(|node| node.valid()).count();
    if valid == 0 {
        let mut reasons: Vec<String> = candidates
            .iter()
            .flat_map(|node| node.reasons.clone())
            .collect();
        reasons.dedup();
        if reasons.is_empty() {
            return format!("0 of {} output node(s) ready", candidates.len());
        }
        return reasons.join(" - ");
    }
    format!("{valid} of {} output node(s) ready", candidates.len())
}

/// Starts a run, or asks first when only some outputs are ready.
pub fn request_run(editor: &mut Editor) {
    let candidates = run_candidates(editor);
    let valid = candidates.iter().filter(|node| node.valid()).count();
    if valid == 0 {
        editor.status = run_tooltip(&candidates);
        return;
    }
    if valid < candidates.len() {
        editor.modal = modals::Modal::RunWorkflow { nodes: candidates };
        return;
    }
    start_run(editor);
}

/// Runs the workflow on a background thread.
pub fn start_run(editor: &mut Editor) {
    let cancelled = Arc::new(AtomicBool::new(false));
    let graph = editor.studio.workflow.graph.clone();
    let registry = editor.studio.registry.clone();
    // Endpoints without an override fall back to the folders their parameters name.
    let placeholder = std::path::Path::new("");
    let mut options =
        editor
            .studio
            .run_options_with_overrides(placeholder, placeholder, &editor.runtime_paths);
    options.cancelled = cancelled.clone();
    let handle = editor.jobs.handle();

    editor.run = Some(ActiveRun {
        cancelled,
        started: Instant::now(),
        node: None,
        file: None,
    });
    editor.progress = modals::Progress::default();
    editor.modal = modals::Modal::BatchProgress;

    std::thread::spawn(move || {
        let mut magick = Magick::discover(options.cancelled.clone());
        let reporter = handle.clone();
        let result = bite_core::execution::run_workflow(
            &graph,
            &registry,
            &mut magick,
            &options,
            &mut |completed, total, path| {
                reporter.send(work::Message::RunProgress {
                    completed,
                    total,
                    file: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_string(),
                });
            },
        );
        match result {
            Ok(batch) => handle.send(work::Message::RunFinished(Box::new(batch))),
            Err(error) => handle.send(work::Message::RunFailed(error)),
        }
    });
}

/// Applies one background message to the editor.
pub fn apply_message(editor: &mut Editor, message: work::Message) {
    match message {
        work::Message::ScanFinished { node, count } => {
            let _ = node;
            editor.inspector.scanning = false;
            editor.inspector.scan_count = Some(count);
        }
        work::Message::ImportProgress {
            completed, total, ..
        } => {
            editor.progress.completed = completed;
            editor.progress.total = total;
        }
        work::Message::ImportFinished {
            node,
            paths,
            thumbnails,
        } => {
            let branch = editor.branches.entry(node.clone()).or_default();
            branch.paths = paths;
            branch.thumbnails = thumbnails
                .iter()
                .map(|thumbnail| crate::panels::filmstrip::Thumbnail {
                    path: thumbnail.path.to_string_lossy().into_owned(),
                    name: crate::panels::filmstrip::display_name(
                        &thumbnail.path.to_string_lossy(),
                    ),
                    texture: None,
                })
                .collect();
            branch.selected = if branch.paths.is_empty() { None } else { Some(0) };
            editor.progress.completed = editor.progress.total;
            editor.pending_uploads = thumbnails
                .into_iter()
                .enumerate()
                .map(|(index, thumbnail)| (format!("thumbnail:{node}:{index}"), thumbnail.image))
                .collect();
            editor.request_preview();
        }
        work::Message::ImportFailed { error, .. } => {
            editor.modal = modals::Modal::Message {
                title: "Bite".into(),
                body: format!("Failed to import images:\n{error}"),
            };
        }
        work::Message::PreviewFinished {
            node,
            index,
            image,
            resolved,
        } => {
            editor.resolved = resolved
                .into_iter()
                .map(|(id, params)| (id, params.into_iter().collect()))
                .collect();
            editor.pending_uploads = vec![(format!("preview:{node}:{index}"), image)];
        }
        work::Message::PreviewFailed(error) => {
            editor.status = error;
        }
        work::Message::TextPreview { lines } => {
            editor.inspector.text_preview_pending = false;
            editor.inspector.text_preview = lines;
        }
        work::Message::RunProgress {
            completed,
            total,
            file,
        } => {
            editor.progress.completed = completed;
            editor.progress.total = total;
            if let Some(run) = &mut editor.run {
                run.file = Some(file);
            }
        }
        work::Message::RunFinished(batch) => {
            let elapsed = editor
                .run
                .as_ref()
                .map(|run| run.started.elapsed().as_millis() as u64);
            editor.run = None;
            editor.modal = modals::Modal::BatchSummary(modals::BatchSummary {
                processed: batch.processed,
                skipped: batch.skipped,
                failed: batch.failed,
                elapsed_ms: elapsed,
                errors: batch.errors.clone(),
                output_dir: batch
                    .outputs
                    .first()
                    .and_then(|path| path.parent())
                    .map(|path| path.to_string_lossy().into_owned()),
            });
        }
        work::Message::RunFailed(error) => {
            editor.run = None;
            editor.progress.error = Some(error);
        }
        work::Message::UpdateChecked(result) => {
            editor.modal = modals::Modal::Update(match *result {
                Ok(info) if info.available => modals::UpdateState::Available {
                    version: info.version,
                    body: info.body,
                    url: info.url,
                },
                Ok(info) => modals::UpdateState::Latest {
                    version: info.version,
                    body: info.body,
                },
                Err(_) => modals::UpdateState::Failed,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(label: &str, reasons: Vec<&str>) -> modals::RunCandidate {
        modals::RunCandidate {
            id: label.into(),
            label: label.into(),
            reasons: reasons.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn the_run_tooltip_reports_an_empty_graph() {
        assert_eq!(run_tooltip(&[]), "No output nodes in graph");
    }

    #[test]
    fn the_run_tooltip_lists_the_reasons_when_nothing_is_ready() {
        let candidates = vec![
            candidate("Image Output", vec!["No image input connected"]),
            candidate("Text Output", vec!["Output file path is empty"]),
        ];
        assert_eq!(
            run_tooltip(&candidates),
            "No image input connected - Output file path is empty"
        );
    }

    #[test]
    fn the_run_tooltip_counts_the_ready_outputs() {
        let candidates = vec![
            candidate("Image Output", vec![]),
            candidate("Text Output", vec!["Output file path is empty"]),
        ];
        assert_eq!(run_tooltip(&candidates), "1 of 2 output node(s) ready");
    }

    #[test]
    fn extension_matching_ignores_case() {
        let extensions = vec!["png".to_string(), "jpg".to_string()];
        assert!(matches_extensions(Path::new("a/b.PNG"), &extensions));
        assert!(!matches_extensions(Path::new("a/b.webp"), &extensions));
        assert!(!matches_extensions(Path::new("a/b"), &extensions));
    }
}
