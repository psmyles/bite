//! Shared execution topology. ImageMagick is an injected process service.
use crate::{
    resolve::{self, ResolvedParams},
    Registry,
};
use bite_expr::{
    definition::{self, CompiledImplementation},
    CompiledArg, Context, Value,
};
use bite_schema::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub trait ImageHost {
    fn continue_after_image_error(&self) -> bool {
        true
    }
    fn run(&mut self, args: &[String]) -> Result<(), String>;
    fn capture(&mut self, args: &[String]) -> Result<String, String>;
    fn metadata(&mut self, path: &Path, heavy: bool) -> Result<Context, String>;
    fn emit(&mut self, operation: OutputOperation, options: &RunOptions) -> Result<(), String> {
        let output = operation.output().to_owned();
        write_output(&output, options, |temporary| match operation {
            OutputOperation::Image { args, format, .. } => {
                self.run(&output_args(args, format.as_deref(), temporary)?)
            }
            OutputOperation::Copy { source, .. } => fs::copy(source, temporary)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            OutputOperation::Text { contents, .. } => {
                fs::write(temporary, contents).map_err(|e| e.to_string())
            }
        })
    }
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputOperation {
    Image {
        args: Vec<String>,
        format: Option<String>,
        output: PathBuf,
    },
    Copy {
        source: PathBuf,
        output: PathBuf,
    },
    Text {
        contents: String,
        output: PathBuf,
    },
}
impl OutputOperation {
    pub fn output(&self) -> &Path {
        match self {
            Self::Image { output, .. } | Self::Copy { output, .. } | Self::Text { output, .. } => {
                output
            }
        }
    }
    pub fn process_args(&self) -> Result<Option<Vec<String>>, String> {
        match self {
            Self::Image {
                args,
                format,
                output,
            } => output_args(args.clone(), format.as_deref(), output).map(Some),
            _ => Ok(None),
        }
    }
}
fn output_args(
    mut args: Vec<String>,
    format: Option<&str>,
    output: &Path,
) -> Result<Vec<String>, String> {
    args.push(if let Some(format) = format {
        format!("{format}:{}", path_text(output)?)
    } else {
        path_text(output)?
    });
    Ok(args)
}
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub named_paths: BTreeMap<String, PathBuf>,
    pub overwrite: bool,
    pub cancelled: Arc<AtomicBool>,
}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BatchResult {
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub outputs: Vec<PathBuf>,
}
#[derive(Debug, Clone)]
struct Stream {
    args: Vec<String>,
    format: Option<String>,
}
fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}
fn text(p: &Context, key: &str, default: &str) -> String {
    p.get(key)
        .map(Value::text)
        .unwrap_or_else(|| default.into())
}
fn scalar(p: &Context, key: &str, default: f64) -> f64 {
    p.get(key)
        .and_then(|v| match v {
            Value::String(s) => s.parse().ok(),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            v => v.scalar().ok(),
        })
        .unwrap_or(default)
}
fn bool_param(p: &Context, key: &str, default: bool) -> bool {
    p.get(key).map(Value::truthy).unwrap_or(default)
}
fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}
fn input_stream(
    node: &GraphNode,
    g: &Graph,
    streams: &BTreeMap<(String, String), Option<Stream>>,
    port: &str,
) -> Option<Stream> {
    let edge = g
        .edges
        .iter()
        .find(|e| e.target == node.id && e.target_handle == format!("in:{port}"))?;
    streams
        .get(&(edge.source.clone(), edge.source_handle.clone()))
        .cloned()
        .flatten()
}
fn filename(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
/// Compare numeric runs by value without integer overflow. Equal runs (including
/// leading zeroes) retain input order through Rust's stable sort.
fn natural_name_cmp(a: &Path, b: &Path) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a = filename(a).to_lowercase();
    let b = filename(b).to_lowercase();
    let (mut a, mut b) = (a.as_str(), b.as_str());
    while !a.is_empty() && !b.is_empty() {
        if a.as_bytes()[0].is_ascii_digit() && b.as_bytes()[0].is_ascii_digit() {
            let al = a.bytes().take_while(u8::is_ascii_digit).count();
            let bl = b.bytes().take_while(u8::is_ascii_digit).count();
            let av = a[..al].trim_start_matches('0');
            let bv = b[..bl].trim_start_matches('0');
            let order = av.len().cmp(&bv.len()).then_with(|| av.cmp(bv));
            if order != Ordering::Equal {
                return order;
            }
            a = &a[al..];
            b = &b[bl..];
        } else {
            let ac = a.chars().next().unwrap();
            let bc = b.chars().next().unwrap();
            let order = ac.cmp(&bc);
            if order != Ordering::Equal {
                return order;
            }
            a = &a[ac.len_utf8()..];
            b = &b[bc.len_utf8()..];
        }
    }
    a.len().cmp(&b.len())
}
pub fn rename(original: &str, blocks: &[RenameBlock], index: usize) -> String {
    if blocks.is_empty() {
        return original.into();
    }
    let split = original.rfind('.').filter(|i| *i > 0);
    let (stem, ext) = split
        .map(|i| (&original[..i], &original[i..]))
        .unwrap_or((original, ""));
    let mut name = String::new();
    for b in blocks {
        match b {
            RenameBlock::Text { value } => name.push_str(value),
            RenameBlock::Number { start, pad } => {
                let number = (start.max(0.0) + 0.5).floor() as usize + index;
                name.push_str(&format!(
                    "{number:0width$}",
                    width = ((*pad + 0.5).floor().max(1.0) as usize).min(4096)
                ));
            }
            RenameBlock::Oldname { find, replace_with } => name.push_str(&if find.is_empty() {
                stem.into()
            } else {
                stem.replace(find, replace_with)
            }),
        }
    }
    name + ext
}
fn image_extensions() -> &'static [&'static str] {
    &[
        "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "svgz", "ico", "bmp", "tif", "tiff",
        "heic", "heif", "jp2", "j2k", "jpf", "jpx", "jxl", "psd", "psb", "exr", "hdr", "dpx",
        "cin", "cr2", "cr3", "nef", "nrw", "arw", "dng", "orf", "raf", "rw2", "pef", "srw", "x3f",
        "3fr", "kdc", "mrw", "erf", "rwl", "tga", "pcx", "ppm", "pgm", "pbm", "pnm", "sgi", "rgb",
        "rgba", "miff", "mng", "jng", "xbm", "xpm", "xwd", "sun", "iff", "lbm", "wbmp", "pict",
        "pct", "dds", "fits", "fts",
    ]
}
fn images(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|_| format!("Cannot read input directory: {}", dir.display()))?
    {
        let e = entry.map_err(|e| e.to_string())?;
        let p = e.path();
        if e.file_type().map_err(|e| e.to_string())?.is_file()
            && p.extension().is_some_and(|e| {
                image_extensions().contains(&e.to_string_lossy().to_lowercase().as_str())
            })
        {
            paths.push(p);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(format!("No images found in: {}", dir.display()));
    }
    Ok(paths)
}
fn mkdir_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Stage beside the destination so a failed/cancelled encoder never destroys an
/// existing output (including an input being overwritten in place).
fn write_output(
    out: &Path,
    options: &RunOptions,
    write: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    mkdir_parent(out)?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let suffix = out
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let temp = tempfile::Builder::new()
        .prefix(".bite-")
        .suffix(&suffix)
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?
        .into_temp_path();
    write(&temp)?;
    if options.cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    if options.overwrite {
        temp.persist(out).map_err(|e| e.to_string())?;
    } else {
        temp.persist_noclobber(out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run_workflow(
    g: &Graph,
    registry: &Registry,
    host: &mut dyn ImageHost,
    options: &RunOptions,
    progress: &mut dyn FnMut(usize, usize, &Path),
) -> Result<BatchResult, String> {
    let plan = crate::plan::build(g, registry)?;
    if plan.outputs.is_empty() {
        return Err("Workflow has no output nodes.".into());
    }
    let mut result = BatchResult::default();
    for output_plan in &plan.outputs {
        if options.cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let output = g.nodes.iter().find(|n| n.id == output_plan.node).unwrap();
        let Some(input_id) = &output_plan.input else {
            continue;
        };
        let input = g.nodes.iter().find(|n| &n.id == input_id).unwrap();
        let input_flag = input
            .data
            .params
            .get("cliName")
            .and_then(definition::to_value)
            .map(|v| v.text())
            .unwrap_or_default();
        let dir=options.named_paths.get(&input_flag).ok_or_else(||format!("Missing required flag: --{}\n  Needed to run output node \"{}\".\n  Export a CLI script from Bite to see all required flags.",if input_flag.is_empty(){"input"}else{&input_flag},output.data.params.get("cliName").and_then(definition::to_value).map(|v|v.text()).unwrap_or_default()))?;
        let files = images(dir)?;
        let raw_output: Context = output
            .data
            .params
            .iter()
            .filter_map(|(k, v)| definition::to_value(v).map(|v| (k.clone(), v)))
            .collect();
        let flag = text(&raw_output, "cliName", "");
        let dest = options
            .named_paths
            .get(&flag)
            .cloned()
            .or_else(|| match output.kind {
                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                    if text(&raw_output, "outputPath", "source") == "custom" {
                        Some(PathBuf::from(text(&raw_output, "customPath", "")))
                    } else {
                        None
                    }
                }
                NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                    Some(PathBuf::from(text(&raw_output, "outputPath", "")))
                }
                _ => Some(PathBuf::from(text(&raw_output, "flipbookOutputPath", ""))),
            });
        if output.kind != NodeKind::Builtin(BuiltinNodeKind::ImageOutput)
            && dest.as_ref().is_some_and(|p| p.exists())
            && !options.overwrite
        {
            result.skipped += files.len();
            continue;
        }
        let contributors: BTreeSet<_> = output_plan.contributors.iter().cloned().collect();
        let heavy = output_plan
            .operations
            .iter()
            .filter_map(|o| registry.nodes.get(&o.definition))
            .flat_map(|d| &d.metadata)
            .any(|m| {
                m == "image.bit_depth"
                    || m.starts_with("image.dpi_")
                    || m.starts_with("image.exif.")
            });
        let set_node = g
            .nodes
            .iter()
            .find(|n| contributors.contains(&n.id) && n.data.definition_id == "process_as_set");
        let mut work: Vec<(PathBuf, Option<String>, BTreeMap<String, PathBuf>)> = Vec::new();
        if let Some(set) = set_node {
            let prefix = set
                .data
                .params
                .get("prefix")
                .and_then(definition::to_value)
                .map(|v| v.text())
                .unwrap_or_default();
            let suffixes = match set.data.params.get("suffixes") {
                Some(ParamValue::Structured(StructuredParam::SetSuffixes { suffixes })) => suffixes,
                _ => return Err("set suffixes missing".into()),
            };
            let mut groups: BTreeMap<String, BTreeMap<String, PathBuf>> = BTreeMap::new();
            for file in &files {
                let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                if let Some(rest) = stem.strip_prefix(&prefix) {
                    for (i, suffix) in suffixes.iter().enumerate() {
                        if !suffix.is_empty() {
                            if let Some(middle) = rest.strip_suffix(suffix) {
                                groups
                                    .entry(middle.into())
                                    .or_default()
                                    .insert(format!("suffix_{i}"), file.clone());
                                break;
                            }
                        }
                    }
                }
            }
            for (name, mut group) in groups {
                let reference = (0..suffixes.len())
                    .find_map(|i| group.get(&format!("suffix_{i}")))
                    .unwrap()
                    .clone();
                // Legacy mat() resolves an unseeded suffix stream to the first
                // available image, including when that stream is transformed.
                for i in 0..suffixes.len() {
                    group
                        .entry(format!("suffix_{i}"))
                        .or_insert_with(|| reference.clone());
                }
                work.push((reference, Some(name), group));
            }
        } else {
            work = files
                .iter()
                .map(|p| (p.clone(), None, BTreeMap::new()))
                .collect();
        }
        let mut report = Vec::new();
        let mut atlas = Vec::new();
        let mut claimed_paths = BTreeSet::new();
        let mut atlas_params = raw_output.clone();
        for (index, (file, set_name, set_inputs)) in work.iter().enumerate() {
            if options.cancelled.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let item_result = (|| -> Result<(), String> {
                let metadata = host.metadata(file, heavy)?;
                let mut resolved = ResolvedParams::new();
                let mut streams: BTreeMap<(String, String), Option<Stream>> = BTreeMap::new();
                // Legacy batch execution selects one input per output. Its lazy
                // materializer falls back to that image for other input nodes;
                // additional input folders are not zipped together at a merge.
                for source in g
                    .nodes
                    .iter()
                    .filter(|n| n.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
                {
                    streams.insert(
                        (source.id.clone(), "out:output".into()),
                        Some(Stream {
                            args: vec![path_text(file)?],
                            format: None,
                        }),
                    );
                }
                for id in &plan.execution_order {
                    if !contributors.contains(id) || id == input_id {
                        continue;
                    }
                    let node = g.nodes.iter().find(|n| &n.id == id).unwrap();
                    let mut params = resolve::node_params(node, g, &resolved, registry, &metadata)?;
                    if node.id == output.id {
                        resolved.insert(id.clone(), params);
                        continue;
                    }
                    let Some(def) = registry.nodes.get(&node.data.definition_id) else {
                        resolved.insert(id.clone(), params);
                        continue;
                    };
                    let incoming = input_stream(node, g, &streams, "input");
                    let mut outgoing = incoming.clone();
                    let enabled = bool_param(&params, "_enabled", true);
                    match &def.implementation {
                        CompiledImplementation::Compute(_) => {}
                        CompiledImplementation::Imagemagick(args) => {
                            if enabled {
                                if let Some(image) = outgoing.as_mut() {
                                    image.args.extend(
                                        CompiledArg::resolve_all(args, &params)
                                            .map_err(|e| e.to_string())?,
                                    );
                                }
                            }
                        }
                        CompiledImplementation::Native(executor) => match executor.as_str() {
                            "gate" => {
                                if enabled && !bool_param(&params, "condition", true) {
                                    outgoing = None;
                                }
                            }
                            // Naming is applied once at the output, matching legacy batch semantics.
                            "rename" => {}
                            "format_convert" => {
                                if enabled {
                                    if let Some(image) = outgoing.as_mut() {
                                        let format = text(&params, "format", "PNG").to_uppercase();
                                        let (def, args) = registry
                                            .formats
                                            .get(&format)
                                            .ok_or_else(|| format!("unknown format {format}"))?;
                                        let mut p = definition::default_context(&def.params);
                                        p.extend(params.clone());
                                        image.args.extend(
                                            CompiledArg::resolve_all(args, &p)
                                                .map_err(|e| e.to_string())?,
                                        );
                                        image.format = Some(format);
                                    }
                                }
                            }
                            "channel_split" => {
                                for (i, name) in ["r", "g", "b", "a"].iter().enumerate() {
                                    let mut channel = incoming.clone();
                                    if let Some(c) = channel.as_mut() {
                                        c.args.extend(s(&[
                                            "-channel",
                                            ["Red", "Green", "Blue", "Alpha"][i],
                                            "-separate",
                                            "+channel",
                                        ]));
                                    }
                                    streams.insert((id.clone(), format!("out:{name}")), channel);
                                }
                                resolved.insert(id.clone(), params);
                                continue;
                            }
                            "channel_merge" => {
                                let mut args = Vec::new();
                                let alpha = scalar(&params, "channels", 3.0) >= 4.0
                                    && g.edges
                                        .iter()
                                        .any(|e| e.target == *id && e.target_handle == "in:a");
                                let mut blocked = false;
                                for name in if alpha {
                                    &["r", "g", "b", "a"][..]
                                } else {
                                    &["r", "g", "b"][..]
                                } {
                                    args.push("(".into());
                                    if let Some(edge) = g.edges.iter().find(|e| {
                                        e.target == *id && e.target_handle == format!("in:{name}")
                                    }) {
                                        if let Some(param) =
                                            edge.source_handle.strip_prefix("param:")
                                        {
                                            let fill = resolved
                                                .get(&edge.source)
                                                .and_then(|p| p.get(param))
                                                .and_then(|v| v.scalar().ok())
                                                .unwrap_or(0.0)
                                                .clamp(0.0, 1.0);
                                            args.push(path_text(file)?);
                                            args.extend(s(&[
                                                "-evaluate",
                                                "set",
                                                &format!("{}%", (fill * 100.0).round()),
                                                "-colorspace",
                                                "Gray",
                                            ]));
                                        } else if let Some(image) = streams
                                            .get(&(edge.source.clone(), edge.source_handle.clone()))
                                            .cloned()
                                            .flatten()
                                        {
                                            args.extend(image.args);
                                        } else {
                                            blocked = true;
                                        }
                                    } else {
                                        args.push(path_text(file)?);
                                        args.extend(s(&[
                                            "-evaluate",
                                            "set",
                                            "0%",
                                            "-colorspace",
                                            "Gray",
                                        ]));
                                    }
                                    args.push(")".into());
                                }
                                args.extend(s(&["-set", "colorspace", "sRGB", "-combine"]));
                                if alpha {
                                    args.extend(s(&["-alpha", "on"]));
                                }
                                outgoing = (!blocked).then_some(Stream { args, format: None });
                            }
                            "mean_value" => {
                                if let Some(image) = incoming {
                                    let mut args = image.args;
                                    args.extend(s(&["-format", "%[fx:mean]", "info:"]));
                                    let value = host
                                        .capture(&args)?
                                        .trim()
                                        .parse::<f64>()
                                        .map_err(|e| e.to_string())?;
                                    params.insert("value".into(), Value::Float(value));
                                }
                            }
                            "process_as_set" => {
                                for (name, path) in set_inputs {
                                    streams.insert(
                                        (id.clone(), format!("out:{name}")),
                                        Some(Stream {
                                            args: vec![path_text(path)?],
                                            format: None,
                                        }),
                                    );
                                }
                                resolved.insert(id.clone(), params);
                                continue;
                            }
                            "solid_image" => {
                                // This source has no image input port. Its canvas
                                // dimensions come from the selected batch input.
                                outgoing = Some(Stream {
                                    args: vec![path_text(file)?],
                                    format: None,
                                });
                                if enabled {
                                    if let Some(image) = outgoing.as_mut() {
                                        image.args.extend(s(&[
                                            "-evaluate",
                                            "set",
                                            &format!("{}%", scalar(&params, "value", 1.0) * 100.0),
                                        ]));
                                    }
                                }
                            }
                            "comment" => {}
                            other => return Err(format!("unimplemented native executor {other}")),
                        },
                    }
                    streams.insert((id.clone(), "out:output".into()), outgoing);
                    resolved.insert(id.clone(), params);
                }
                let output_params = resolved.get(&output.id).unwrap_or(&raw_output);
                let image = input_stream(output, g, &streams, "input");
                match output.kind {
                    NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                        let Some(image) = image else {
                            result.skipped += 1;
                            return Ok(());
                        };
                        // Legacy batch naming uses the first rename in global topological
                        // order, including disabled or disconnected rename nodes.
                        let mut name = filename(file);
                        if let Some(node) = plan
                            .execution_order
                            .iter()
                            .filter_map(|id| g.nodes.iter().find(|n| n.id == *id))
                            .find(|n| n.data.definition_id == "rename")
                        {
                            if let Some(ParamValue::Structured(StructuredParam::RenameBlocks {
                                blocks,
                            })) = node.data.params.get("blocks")
                            {
                                name = rename(&name, blocks, index);
                            }
                        }
                        if let Some(set) = set_name {
                            name = format!(
                                "{}{}{}.{}",
                                text(output_params, "setOutputPrefix", ""),
                                set,
                                text(output_params, "setOutputSuffix", ""),
                                file.extension().unwrap_or_default().to_string_lossy()
                            );
                        }
                        if let Some(format) = &image.format {
                            let ext = &registry.formats[format].0.extension;
                            name = Path::new(&name)
                                .with_extension(ext.trim_start_matches('.'))
                                .to_string_lossy()
                                .into_owned();
                        }
                        if Path::new(&name).file_name().and_then(|s| s.to_str()) != Some(&name) {
                            return Err("rename result must be a filename".into());
                        }
                        let desired = dest
                            .clone()
                            .unwrap_or_else(|| file.parent().unwrap().into())
                            .join(name);
                        let mut out = desired.clone();
                        let mut collision = 1;
                        while !claimed_paths.insert(out.clone()) {
                            let stem = desired.file_stem().unwrap_or_default().to_string_lossy();
                            let ext = desired
                                .extension()
                                .map(|e| format!(".{}", e.to_string_lossy()))
                                .unwrap_or_default();
                            out = desired.with_file_name(format!("{stem}_{collision}{ext}"));
                            collision += 1;
                        }
                        if out.exists() && !options.overwrite {
                            result.skipped += 1;
                            return Ok(());
                        }
                        let operation = if image.args.len() == 1 && image.format.is_none() {
                            OutputOperation::Copy {
                                source: image.args[0].clone().into(),
                                output: out.clone(),
                            }
                        } else {
                            OutputOperation::Image {
                                args: image.args,
                                format: image.format,
                                output: out.clone(),
                            }
                        };
                        host.emit(operation, options)?;
                        result.processed += 1;
                        result.outputs.push(out);
                    }
                    NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                        if image.is_none() || !bool_param(output_params, "_txo_condition", true) {
                            result.skipped += 1;
                            return Ok(());
                        }
                        let slots = match output.data.params.get("portIds") {
                            Some(ParamValue::Structured(StructuredParam::TextSlots { slots })) => {
                                slots.as_slice()
                            }
                            _ => &[],
                        };
                        let fields: Vec<_> = slots
                            .iter()
                            .take(slots.len().saturating_sub(1))
                            .filter_map(|slot| output_params.get(&format!("_txo_{slot}")))
                            .map(resolve::text_value)
                            .collect();
                        let sep = match text(output_params, "separatorType", "comma").as_str() {
                            "tab" => "\t".into(),
                            "space" => " ".into(),
                            "custom" => text(output_params, "customSeparator", ""),
                            _ => ",".into(),
                        };
                        report.push(fields.join(&sep));
                        result.processed += 1;
                    }
                    NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
                        atlas.push(file.clone());
                        atlas_params = output_params.clone();
                    }
                    _ => {}
                }
                Ok(())
            })();
            if let Err(error) = item_result {
                if options.cancelled.load(Ordering::Relaxed) || !host.continue_after_image_error() {
                    return Err(error);
                }
                result.failed += 1;
                result.errors.push(format!(
                    "{} (output {}): {error}",
                    file.display(),
                    output.id
                ));
            }
            progress(index + 1, work.len(), file);
        }
        if output.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput) {
            let out = dest.clone().ok_or("text output path missing")?;
            if out.as_os_str().is_empty() {
                return Err("text output path empty".into());
            }
            host.emit(
                OutputOperation::Text {
                    output: out.clone(),
                    contents: report.join("\n") + if report.is_empty() { "" } else { "\n" },
                },
                options,
            )?;
            result.outputs.push(out);
        }
        if output.kind == NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) {
            let out = dest.ok_or("flipbook output path missing")?;
            let rows = scalar(&atlas_params, "rows", 4.0) as usize;
            let cols = scalar(&atlas_params, "cols", 4.0) as usize;
            if rows == 0 || cols == 0 || rows.saturating_mul(cols) > 65536 {
                return Err("invalid atlas grid".into());
            }
            let sort = text(&atlas_params, "sortBy", "import_order");
            if sort == "name" {
                atlas.sort_by(|a, b| natural_name_cmp(a, b));
            } else if sort == "name_desc" {
                atlas.sort_by(|a, b| natural_name_cmp(b, a));
            }
            atlas.truncate(rows * cols);
            let mut args = vec!["montage".into()];
            for p in &atlas {
                args.push(path_text(p)?);
            }
            let bg = atlas_params
                .get("bgColor")
                .cloned()
                .unwrap_or(Value::Vector(vec![0.0; 4]));
            let color = if let Value::Vector(v) = bg {
                if v.len() == 4 && v[3] >= 0.01 {
                    format!(
                        "#{:02x}{:02x}{:02x}{:02x}",
                        (v[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                        (v[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                        (v[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                        (v[3].clamp(0.0, 1.0) * 255.0).round() as u8
                    )
                } else {
                    "none".into()
                }
            } else {
                "none".into()
            };
            args.extend(s(&[
                "-tile",
                &format!("{cols}x{rows}"),
                "-geometry",
                &format!(
                    "{}x{}!+0+0",
                    scalar(&atlas_params, "cellWidth", 128.0),
                    scalar(&atlas_params, "cellHeight", 128.0)
                ),
                "-background",
                &color,
            ]));
            host.emit(
                OutputOperation::Image {
                    args,
                    format: None,
                    output: out.clone(),
                },
                options,
            )?;
            result.processed += atlas.len();
            result.outputs.push(out);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_outputs_preserve_existing_files_on_failure_cancel_and_skip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        fs::write(&path, b"original").unwrap();
        let options = RunOptions {
            overwrite: true,
            ..Default::default()
        };
        let result = write_output(&path, &options, |temp| {
            fs::write(temp, b"partial").unwrap();
            Err("encoder failed".into())
        });
        assert!(result.unwrap_err().contains("encoder failed"));
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let result = write_output(&path, &options, |temp| {
            fs::write(temp, b"cancelled").unwrap();
            options.cancelled.store(true, Ordering::Relaxed);
            Ok(())
        });
        assert_eq!(result.unwrap_err(), "Cancelled");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(write_output(&path, &RunOptions::default(), |temp| {
            fs::write(temp, b"skipped").map_err(|e| e.to_string())
        })
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        options.cancelled.store(false, Ordering::Relaxed);
        write_output(&path, &options, |temp| {
            fs::write(temp, b"complete").map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "temporary files must be cleaned up"
        );
    }
}
