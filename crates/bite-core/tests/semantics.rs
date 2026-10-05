//! Run and preview semantics measured against what the Electron pipeline did, each with a
//! host that records what it was asked instead of spawning ImageMagick.
use bite_core::{
    execution::{self, BatchResult, ImageHost, RunOptions},
    graph, preview, workflow, Registry,
};
use bite_expr::{Context, Value};
use bite_schema::Graph;
use serde_json::{json, Value as Json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> Registry {
    Registry::load(
        &root().join("node-definitions"),
        &root().join("format-definitions"),
    )
    .unwrap()
}

/// Records every process and measurement, writes whatever output it is handed, and
/// answers metadata the way the real host does: the header for the common formats, the
/// full inspection when asked for it.
#[derive(Default)]
struct Host {
    /// What a full inspection reports for every file.
    size: (i64, i64),
    /// The header parser cannot read these files, so the cheap read reports no size.
    unreadable_header: bool,
    heavy_reads: usize,
    runs: Vec<Vec<String>>,
    captures: Vec<Vec<String>>,
}
impl ImageHost for Host {
    fn run(&mut self, args: &[String]) -> Result<(), String> {
        self.runs.push(args.to_vec());
        let output = args.last().ok_or("no output")?;
        // `PNG:path` names the encoder; a drive letter is not one.
        let output = match output.split_once(':') {
            Some((format, path))
                if format.len() > 1 && format.chars().all(|c| c.is_ascii_uppercase()) =>
            {
                path
            }
            _ => output,
        };
        fs::write(output, b"rendered").map_err(|error| error.to_string())
    }
    fn capture(&mut self, args: &[String]) -> Result<String, String> {
        self.captures.push(args.to_vec());
        let has = |arg: &str| args.iter().any(|item| item == arg);
        Ok(if has("-separate") && !has("-channel") {
            "0.1 0.2 0.3 "
        } else {
            "0.5"
        }
        .into())
    }
    fn metadata(&mut self, path: &Path, heavy: bool) -> Result<Context, String> {
        if heavy {
            self.heavy_reads += 1;
        }
        let (width, height) = if heavy || !self.unreadable_header {
            self.size
        } else {
            (0, 0)
        };
        Ok(Context::from([
            ("image.width".into(), Value::Int(width)),
            ("image.height".into(), Value::Int(height)),
            (
                "image.path".into(),
                Value::String(path.display().to_string()),
            ),
            (
                "image.name".into(),
                Value::String(path.file_name().unwrap().to_string_lossy().into_owned()),
            ),
        ]))
    }
}
impl Host {
    fn sized(width: i64, height: i64) -> Self {
        Self {
            size: (width, height),
            ..Self::default()
        }
    }
    /// The command that ran this operation, by one of its arguments.
    fn run_with(&self, argument: &str) -> &[String] {
        self.runs
            .iter()
            .find(|args| args.iter().any(|a| a == argument))
            .unwrap_or_else(|| panic!("nothing ran with {argument}: {:?}", self.runs))
    }
}

fn node(id: &str, kind: &str, definition: &str, params: Json) -> Json {
    json!({
        "id": id,
        "type": kind,
        "position": {"x": 0, "y": 0},
        "data": {"label": id, "definitionId": definition, "params": params},
    })
}
fn process(id: &str, definition: &str, params: Json) -> Json {
    node(id, "process", definition, params)
}
fn input() -> Json {
    node("input", "inputNode", "", json!({"cliName": "in"}))
}
fn image_output() -> Json {
    node(
        "output",
        "imageOutputNode",
        "",
        json!({"cliName": "out", "outputPath": "source", "overwrite": "overwrite"}),
    )
}
fn flipbook(dir: &Path, params: Json) -> Json {
    let mut defaults = json!({
        "cliName": "atlas",
        "flipbookOutputPath": dir.join("atlas.png").display().to_string(),
        "cols": 4, "rows": 4, "cellWidth": 64, "cellHeight": 64,
        "sortBy": "import_order", "bgColor": [0, 0, 0, 0],
        "overwrite": "overwrite", "generateLog": false,
    });
    defaults
        .as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    node("flipbook", "flipbookOutputNode", "", defaults)
}
fn set_node(prefix: &str, suffixes: &[&str]) -> Json {
    let mut set = process(
        "set",
        "process_as_set",
        json!({"prefix": prefix, "suffixes": {"type": "set_suffixes", "suffixes": suffixes}}),
    );
    set["data"]["outputs"] = suffixes
        .iter()
        .enumerate()
        .map(|(i, label)| json!({"name": format!("suffix_{i}"), "type": "image", "label": label}))
        .collect();
    set
}
fn graph(nodes: Vec<Json>, edges: &[(&str, &str, &str, &str)]) -> Graph {
    let edges: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(i, (source, source_handle, target, target_handle))| {
            json!({
                "id": format!("e{i}"),
                "source": source, "sourceHandle": source_handle,
                "target": target, "targetHandle": target_handle,
            })
        })
        .collect();
    let graph: Graph = serde_json::from_value(json!({
        "nodes": nodes, "edges": edges, "viewport": {"x": 0, "y": 0, "zoom": 1},
    }))
    .unwrap();
    graph::validate(&graph, &registry()).unwrap();
    graph
}
fn images(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            fs::write(&path, name.as_bytes()).unwrap();
            path
        })
        .collect()
}
/// Runs over exactly these files, in this order, as the interface hands over an import.
fn run(graph: &Graph, host: &mut Host, files: &[PathBuf], out: &Path) -> BatchResult {
    let options = RunOptions {
        named_files: [("in".into(), files.to_vec())].into(),
        named_paths: [("out".into(), out.to_owned())].into(),
        ..RunOptions::default()
    };
    execution::run_workflow(graph, &registry(), host, &options, &mut |_, _, _| {}).unwrap()
}
/// The files a montage was given, in the order it tiles them.
fn tiled(host: &Host) -> Vec<String> {
    let montage = host.run_with("montage");
    let end = montage.iter().position(|a| a == "-tile").unwrap();
    montage[1..end]
        .iter()
        .map(|p| {
            Path::new(p)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// Electron compared with `numericScalar`, so text that is not a number was NaN: never
/// equal, always "not equal", and never an error. Failing here took down every preview of
/// the workflow, even with the Compare wired to nothing.
#[test]
fn comparing_text_is_false_rather_than_an_error() {
    let registry = registry();
    let compare = |operator: &str| {
        graph(
            vec![process(
                "compare",
                "logic_comparison",
                json!({"a": "abc", "b": "abc", "operator": operator}),
            )],
            &[],
        )
    };
    for (operator, expected) in [
        ("equal", false),
        ("not equal", true),
        ("greater than", false),
        ("less or equal", false),
    ] {
        let values = preview::values(&compare(operator), &registry).unwrap();
        assert_eq!(
            values["compare"]["result"],
            Value::Bool(expected),
            "{operator}"
        );
    }

    // A disconnected Compare does not stop the image beside it from previewing.
    let dir = tempfile::tempdir().unwrap();
    let [image] = images(dir.path(), &["photo.png"]).try_into().unwrap();
    let graph = graph(
        vec![
            input(),
            process("negate", "negate", json!({})),
            process(
                "compare",
                "logic_comparison",
                json!({"a": "abc", "b": 3, "operator": "less than"}),
            ),
        ],
        &[("input", "out:output", "negate", "in:input")],
    );
    let result = preview::render(
        &graph,
        &registry,
        &mut Host::sized(64, 64),
        &image,
        Some(("negate", "out:output")),
    )
    .unwrap();
    assert!(result.png.is_some());
    assert_eq!(
        result.resolved_values["compare"]["result"],
        Value::Bool(false)
    );
}

/// A value node that cannot work its output out keeps what it holds, as Electron's
/// `computeNodeParams` caught the failure, and the nodes around it still resolve.
#[test]
fn a_failing_value_node_leaves_the_rest_of_the_graph_resolved() {
    let mut registry = registry();
    let broken: bite_schema::NodeDefinition = serde_json::from_value(json!({
        "id": "test_broken", "version": "1.0.0", "label": "Broken", "category": "Test",
        "description": "Converts text that is not an integer", "schema_version": 2,
        "inputs": [], "outputs": [],
        "params": [
            {"name": "text", "label": "Text", "type": "string", "default": "abc"},
            {"name": "n", "label": "N", "type": "int", "default": 7, "readonly": true},
        ],
        "implementation": {"type": "compute", "outputs": {"n": "int(text)"}},
    }))
    .unwrap();
    registry.nodes.insert(
        "test_broken".into(),
        bite_expr::definition::CompiledDefinition::compile(broken).unwrap(),
    );
    let graph: Graph = serde_json::from_value(json!({
        "nodes": [
            process("broken", "test_broken", json!({})),
            process("float", "value_float", json!({"value": 2})),
            process("add", "math_add", json!({"b": 3})),
        ],
        "edges": [{"id": "e", "source": "float", "sourceHandle": "param:value",
                   "target": "add", "targetHandle": "param:a"}],
        "viewport": {"x": 0, "y": 0, "zoom": 1},
    }))
    .unwrap();
    let values = preview::values(&graph, &registry).unwrap();
    assert_eq!(values["add"]["result"], Value::Float(5.0));
    assert_eq!(values["broken"]["n"], Value::Int(7));
}

/// A Flipbook tiles the images in the order they were imported, and a Rename numbers
/// them that way, as Electron took `imagePaths` as given; a file listed twice counts once.
#[test]
fn listed_files_keep_their_import_order() {
    let dir = tempfile::tempdir().unwrap();
    let [a, b, c] = images(dir.path(), &["a.png", "b.png", "c.png"])
        .try_into()
        .unwrap();
    let order = [c.clone(), a.clone(), b.clone(), a.clone()];

    let atlas = graph(
        vec![input(), flipbook(dir.path(), json!({}))],
        &[("input", "out:output", "flipbook", "in:input")],
    );
    let mut host = Host::sized(64, 64);
    run(&atlas, &mut host, &order, dir.path());
    assert_eq!(tiled(&host), ["c.png", "a.png", "b.png"]);

    let renamed = graph(
        vec![
            input(),
            process(
                "rename",
                "rename",
                json!({"blocks": {"type": "rename_blocks", "blocks": [
                    {"type": "number", "start": 1, "pad": 1},
                    {"type": "text", "value": "_"},
                    {"type": "oldname", "find": "", "replace_with": ""},
                ]}}),
            ),
            image_output(),
        ],
        &[
            ("input", "out:output", "rename", "in:input"),
            ("rename", "out:output", "output", "in:input"),
        ],
    );
    let out = dir.path().join("renamed");
    let result = run(&renamed, &mut Host::sized(64, 64), &order, &out);
    let names: Vec<_> = result
        .outputs
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["1_c.png", "2_a.png", "3_b.png"]);
}

/// Electron's text batch wrote `<path>.txt` when the path did not already end that way,
/// and looked for an existing file under that name too.
#[test]
fn a_text_output_is_written_with_a_txt_extension() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["a.png"]);
    let report = |path: &Path, overwrite: &str| {
        graph(
            vec![
                input(),
                process("value", "value_string", json!({"value": "line"})),
                node(
                    "text",
                    "textOutputNode",
                    "",
                    json!({
                        "cliName": "report", "outputPath": path.display().to_string(),
                        "overwrite": overwrite, "separatorType": "comma",
                        "portIds": {"type": "text_slots", "slots": ["0", "1"]},
                    }),
                ),
            ],
            &[
                ("input", "out:output", "text", "in:input"),
                ("value", "param:value", "text", "txo:0"),
            ],
        )
    };
    let result = run(
        &report(&dir.path().join("report"), "overwrite"),
        &mut Host::default(),
        &files,
        dir.path(),
    );
    let written = dir.path().join("report.txt");
    assert_eq!(result.outputs, std::slice::from_ref(&written));
    assert_eq!(fs::read_to_string(&written).unwrap(), "line\n");

    // A name that already ends in .txt, in any case, is left as it is.
    let upper = dir.path().join("Upper.TXT");
    let result = run(
        &report(&upper, "overwrite"),
        &mut Host::default(),
        &files,
        dir.path(),
    );
    assert_eq!(result.outputs, [upper]);

    // The existing report.txt is what a "skip" output finds and leaves alone.
    fs::write(&written, "kept").unwrap();
    let result = run(
        &report(&dir.path().join("report"), "skip"),
        &mut Host::default(),
        &files,
        dir.path(),
    );
    assert_eq!(result.skipped, 1);
    assert_eq!(fs::read_to_string(&written).unwrap(), "kept");
}

/// A Logic Branch whose chosen side is unwired passes on nothing. Electron read that as
/// zero in arithmetic and as empty text, so the nodes downstream still resolve.
#[test]
fn nothing_on_a_wire_reads_as_zero_or_empty_text() {
    let graph = graph(
        vec![
            process("branch", "logic_branch", json!({"condition": true})),
            process("add", "math_add", json!({"b": 2})),
            process("split", "split_vec", json!({})),
            process("power", "math_power", json!({"exponent": 2})),
            // Read as the text "null", the input would contain what it is filtered for.
            process("text", "text_filter", json!({"contains": "null"})),
        ],
        &[
            ("branch", "param:result", "add", "param:a"),
            ("branch", "param:result", "split", "param:vec"),
            ("branch", "param:result", "power", "param:base"),
            ("branch", "param:result", "text", "param:input"),
        ],
    );
    let values = preview::values(&graph, &registry()).unwrap();
    assert_eq!(values["branch"]["result"], Value::Null);
    assert_eq!(values["add"]["result"], Value::Float(2.0));
    assert_eq!(values["split"]["x"], Value::Float(0.0));
    assert_eq!(values["power"]["result"], Value::Float(0.0));
    assert_eq!(values["text"]["input"], Value::String(String::new()));
    assert_eq!(values["text"]["result"], Value::Bool(false));
}

/// A Color node wired into a colour parameter arrives as components from 0 to 1, which
/// ImageMagick cannot read as `1,0,0,1`; it is written as an `rgba()` colour instead.
#[test]
fn a_wired_color_is_a_colour_imagemagick_reads() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["a.png"]);
    for (definition, param, flag) in [
        ("tint", "color", "-fill"),
        ("outline", "color", "-background"),
        ("rotate", "background", "-background"),
    ] {
        let graph = graph(
            vec![
                input(),
                process("color", "value_color", json!({"color": [1, 0, 0, 0.5]})),
                process("op", definition, json!({})),
                image_output(),
            ],
            &[
                ("input", "out:output", "op", "in:input"),
                ("color", "param:rgba", "op", &format!("param:{param}")),
                ("op", "out:output", "output", "in:input"),
            ],
        );
        let mut host = Host::sized(64, 64);
        run(&graph, &mut host, &files, &dir.path().join(definition));
        let args = host.run_with(flag);
        let colour = &args[args.iter().position(|a| a == flag).unwrap() + 1];
        assert_eq!(colour, "rgba(255,0,0,0.5)", "{definition}");
    }
}

/// A Gate that shuts on a file keeps that file out of the atlas.
#[test]
fn a_flipbook_leaves_out_the_files_a_gate_shut() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["a.png", "b.png", "c.png"]);
    let graph = graph(
        vec![
            input(),
            process("name", "prop_name", json!({})),
            process("filter", "text_filter", json!({"prefix": "b"})),
            process("gate", "gate", json!({})),
            flipbook(dir.path(), json!({})),
        ],
        &[
            ("input", "out:output", "gate", "in:input"),
            ("name", "param:value", "filter", "param:input"),
            ("filter", "param:result", "gate", "param:condition"),
            ("gate", "out:output", "flipbook", "in:input"),
        ],
    );
    let mut host = Host::sized(64, 64);
    let result = run(&graph, &mut host, &files, dir.path());
    assert_eq!(tiled(&host), ["b.png"]);
    assert_eq!((result.processed, result.skipped), (1, 2));
}

/// The header parser reads no size from a TIFF or a PSD. A preview already inspected such
/// a file fully; a run now does too, so a Resize wired from Dimensions runs at the real
/// width instead of one pixel. Nothing pays for it when nothing reads the size.
#[test]
fn a_run_inspects_a_file_whose_header_gives_no_size() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["scan.tif"]);
    let resize = graph(
        vec![
            input(),
            process("dims", "prop_dimensions", json!({})),
            process("resize", "resize", json!({})),
            image_output(),
        ],
        &[
            ("input", "out:output", "resize", "in:input"),
            ("dims", "param:width", "resize", "param:width"),
            ("resize", "out:output", "output", "in:input"),
        ],
    );
    let mut host = Host {
        unreadable_header: true,
        ..Host::sized(4000, 3000)
    };
    run(&resize, &mut host, &files, &dir.path().join("resized"));
    let args = host.run_with("-resize");
    assert_eq!(
        args[args.iter().position(|a| a == "-resize").unwrap() + 1],
        "4000"
    );

    let negate = graph(
        vec![
            input(),
            process("negate", "negate", json!({})),
            image_output(),
        ],
        &[
            ("input", "out:output", "negate", "in:input"),
            ("negate", "out:output", "output", "in:input"),
        ],
    );
    let mut host = Host {
        unreadable_header: true,
        ..Host::sized(4000, 3000)
    };
    run(&negate, &mut host, &files, &dir.path().join("negated"));
    assert_eq!(host.heavy_reads, 0);
}

/// Previewing a file the set's pattern does not match shows that file on every suffix
/// port, as the Electron preview did, while a run still leaves the file out.
#[test]
fn a_set_preview_of_an_unmatched_file_shows_that_file() {
    let dir = tempfile::tempdir().unwrap();
    let [photo] = images(dir.path(), &["photo.png"]).try_into().unwrap();
    let graph = graph(
        vec![
            input(),
            set_node("set_", &["_diffuse", "_normal"]),
            process("negate", "negate", json!({})),
            image_output(),
        ],
        &[
            ("input", "out:output", "set", "in:input"),
            ("set", "out:suffix_1", "negate", "in:input"),
            ("negate", "out:output", "output", "in:input"),
        ],
    );
    let mut host = Host::sized(64, 64);
    let result = preview::render(
        &graph,
        &registry(),
        &mut host,
        &photo,
        Some(("negate", "out:output")),
    )
    .unwrap();
    assert!(result.png.is_some());
    assert!(host.run_with("-negate")[0].ends_with("photo.png"));

    let mut host = Host::sized(64, 64);
    let result = run(&graph, &mut host, &[photo], &dir.path().join("out"));
    assert_eq!(result.processed, 0);
    assert!(host.runs.is_empty());
}

/// A boolean wired into a Channel Merge channel fills it: `Number(true)` is 1, so 100%.
#[test]
fn a_wired_boolean_fills_a_merged_channel() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["a.png"]);
    let graph = graph(
        vec![
            input(),
            process("on", "value_boolean", json!({"value": true})),
            process("merge", "channel_merge", json!({})),
            image_output(),
        ],
        &[
            ("on", "param:value", "merge", "in:r"),
            ("input", "out:output", "merge", "in:g"),
            ("merge", "out:output", "output", "in:input"),
        ],
    );
    let mut host = Host::sized(64, 64);
    run(&graph, &mut host, &files, &dir.path().join("out"));
    assert!(host.run_with("-combine").contains(&"100%".to_string()));
}

/// With nothing wired into it, a Mean Value measures the image being processed, as
/// Electron's fallback to the input did, and measures only its first frame.
#[test]
fn an_unwired_mean_value_measures_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let [image] = images(dir.path(), &["anim.gif"]).try_into().unwrap();
    let graph = graph(
        vec![
            input(),
            process("mean", "mean_value", json!({})),
            process(
                "compare",
                "logic_comparison",
                json!({"b": 0.4, "operator": "greater than"}),
            ),
            process("gate", "gate", json!({})),
            image_output(),
        ],
        &[
            ("input", "out:output", "gate", "in:input"),
            ("mean", "param:value", "compare", "param:a"),
            ("compare", "param:result", "gate", "param:condition"),
            ("gate", "out:output", "output", "in:input"),
        ],
    );
    let mut host = Host::sized(64, 64);
    let result = run(
        &graph,
        &mut host,
        std::slice::from_ref(&image),
        &dir.path().join("out"),
    );
    assert_eq!(result.processed, 1, "the mean of 0.5 opened the gate");
    assert_eq!(
        host.captures[0][0],
        format!("{}[0]", image.display()),
        "{:?}",
        host.captures
    );
}

/// A prefix wired into a Process As Set picks the files, and names the set, by the value
/// the wire carries - worked out from upstream, where the source stores nothing itself.
#[test]
fn a_wired_set_prefix_groups_and_names_the_set() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(
        dir.path(),
        &["set_alpha_diffuse.png", "set_alpha_normal.png", "other.png"],
    );
    let graph = graph(
        vec![
            input(),
            process("typed", "value_string", json!({"value": "set_"})),
            process("branch", "logic_branch", json!({"condition": true})),
            set_node("zzz_", &["_diffuse", "_normal"]),
            process("negate", "negate", json!({})),
            image_output(),
        ],
        &[
            ("input", "out:output", "set", "in:input"),
            ("typed", "param:value", "branch", "param:value_true"),
            ("branch", "param:result", "set", "param:prefix"),
            ("set", "out:suffix_0", "negate", "in:input"),
            ("negate", "out:output", "output", "in:input"),
        ],
    );
    let out = dir.path().join("out");
    let result = run(&graph, &mut Host::sized(64, 64), &files, &out);
    assert_eq!(result.outputs, [out.join("set_alpha.png")]);
}

/// The grid and the cell size of a Flipbook take wired numbers, through the ports Electron
/// offered and the names its v1 files saved them under.
#[test]
fn a_flipbook_grid_takes_wired_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let files = images(dir.path(), &["a.png", "b.png", "c.png"]);
    let graph = graph(
        vec![
            input(),
            process("two", "value_float", json!({"value": 2})),
            process("dims", "prop_dimensions", json!({})),
            flipbook(dir.path(), json!({})),
        ],
        &[
            ("input", "out:output", "flipbook", "in:input"),
            ("two", "param:value", "flipbook", "param:cols"),
            ("dims", "param:width", "flipbook", "param:cellWidth"),
        ],
    );
    let mut host = Host::sized(300, 200);
    run(&graph, &mut host, &files, dir.path());
    let montage = host.run_with("montage");
    let after = |flag: &str| &montage[montage.iter().position(|a| a == flag).unwrap() + 1];
    assert_eq!(after("-tile"), "2x4");
    assert_eq!(after("-geometry"), "300x64!+0+0");

    let mut v1: Json = serde_json::from_str(
        &fs::read_to_string(root().join("test-workflows/wf-10-flipbook.bite")).unwrap(),
    )
    .unwrap();
    v1["graph"]["nodes"].as_array_mut().unwrap().push(json!({
        "id": "rows-1", "type": "process", "position": {"x": 0, "y": 0},
        "data": {"label": "Rows", "definitionId": "value_float", "params": {"value": 3}},
    }));
    v1["graph"]["edges"].as_array_mut().unwrap().push(json!({
        "id": "e-rows", "source": "rows-1", "sourceHandle": "param-out-value",
        "target": "flipbook-1", "targetHandle": "param-in-rows",
    }));
    let loaded = workflow::load(&v1.to_string(), &registry()).unwrap();
    assert!(loaded
        .workflow
        .graph
        .edges
        .iter()
        .any(|e| e.source == "rows-1" && e.target_handle == "param:rows"));
}

/// A text preview reads the images through temporary links, but a Path in its lines is
/// the image the user picked.
#[test]
fn a_text_preview_reports_the_real_path() {
    let dir = tempfile::tempdir().unwrap();
    let [image] = images(dir.path(), &["photo.png"]).try_into().unwrap();
    let graph = graph(
        vec![
            input(),
            process("path", "prop_path", json!({})),
            process("folder", "prop_path", json!({"strip_filename": true})),
            node(
                "text",
                "textOutputNode",
                "",
                json!({
                    "cliName": "report", "outputPath": "", "overwrite": "skip",
                    "separatorType": "comma",
                    "portIds": {"type": "text_slots", "slots": ["0", "1", "2"]},
                }),
            ),
        ],
        &[
            ("input", "out:output", "text", "in:input"),
            ("path", "param:value", "text", "txo:0"),
            ("folder", "param:value", "text", "txo:1"),
        ],
    );
    let lines = preview::render_text(
        &graph,
        &registry(),
        &mut Host::sized(64, 64),
        std::slice::from_ref(&image),
        "text",
    )
    .unwrap();
    assert_eq!(
        lines,
        [format!(
            "{},{}",
            image.display(),
            image.parent().unwrap().display()
        )]
    );
}

/// The canvas shows an unwired Mean Value measuring the previewed image, as Electron's
/// preview fell back to the selected file, rather than a zero it never measured.
#[test]
fn a_preview_reports_an_unwired_mean() {
    let dir = tempfile::tempdir().unwrap();
    let [image] = images(dir.path(), &["photo.png"]).try_into().unwrap();
    let graph = graph(
        vec![
            input(),
            process("negate", "negate", json!({})),
            process("mean", "mean_value", json!({})),
        ],
        &[("input", "out:output", "negate", "in:input")],
    );
    let result = preview::render(
        &graph,
        &registry(),
        &mut Host::sized(64, 64),
        &image,
        Some(("negate", "out:output")),
    )
    .unwrap();
    assert_eq!(result.resolved_values["mean"]["value"], Value::Float(0.5));
}
