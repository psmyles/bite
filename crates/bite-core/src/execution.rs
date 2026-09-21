//! Shared execution topology. ImageMagick is an injected process service.
use crate::{
    graph,
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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub trait ImageHost {
    fn continue_after_image_error(&self) -> bool {
        true
    }
    fn image_error(&mut self, _input: &Path, _output: &str, _error: &str) {}
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
    fn emit_many(
        &mut self,
        operations: Vec<OutputOperation>,
        options: &RunOptions,
        _jobs: usize,
    ) -> Vec<Result<(), String>> {
        let mut results = Vec::with_capacity(operations.len());
        for operation in operations {
            if options.cancelled.load(Ordering::Relaxed) {
                results.push(Err("Cancelled".into()));
            } else {
                results.push(self.emit(operation, options));
            }
        }
        results
    }
    /// Independent hosts that may resolve files at the same time as this one.
    ///
    /// Resolving a file reads its metadata and runs whatever analysis its chain asks for,
    /// such as a mean or a comparison, and for a workflow like a channel gate that is
    /// nearly all of the run. Those files never read what another writes, so a host that
    /// can spawn concurrent processes hands back workers here and the batch is split
    /// across them.
    ///
    /// Returning nothing, which is the default, keeps every file on this host in order.
    /// That is what a host that records rather than spawns wants, and it is what makes a
    /// plan reproducible.
    fn workers(&self, _jobs: usize) -> Vec<Box<dyn ImageHost + Send>> {
        Vec::new()
    }
}
/// What one input file resolved to for one output node, before the results are folded
/// back together in file order.
///
/// Nothing here has touched shared state: output paths are still names, and the counters
/// a run reports are still the caller's to add up. That is what lets the files be
/// resolved in any order and still land exactly as a serial run would have left them.
pub struct ItemResult {
    output: ItemOutput,
    /// `capture_values` only: what every value node read on this file.
    values: BTreeMap<String, Context>,
    /// `capture_values` only: the nodes that stopped the stream.
    stopped: Vec<String>,
}
pub type ItemOutcome = Result<ItemResult, String>;
enum ItemOutput {
    /// The chain reached this output with nothing to write.
    Skipped,
    /// The output takes no per-file work, or this file contributed none.
    None,
    /// The command this file resolved to, under the name it wants. The directory and
    /// any collision suffix are settled when the results are merged.
    Image {
        name: String,
        args: Vec<String>,
        format: Option<String>,
    },
    Text(String),
    Atlas(Context),
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
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub named_paths: BTreeMap<String, PathBuf>,
    /// The exact files an input flag should process, for a host that already holds a
    /// list (the interface imports a selection) instead of a folder to enumerate.
    pub named_files: BTreeMap<String, Vec<PathBuf>>,
    /// Files whose *measurements* come from another file than the one being processed.
    ///
    /// A preview renders a thumbnail for speed, so what it hands the pipeline is small,
    /// while a parameter written as a share of the image's width must still mean a share
    /// of the real one. Mapping the staged thumbnail to the original makes every property,
    /// dimension and expression resolve against the picture the user actually selected.
    pub measured_as: BTreeMap<PathBuf, PathBuf>,
    pub overwrite: bool,
    pub cancelled: Arc<AtomicBool>,
    pub analysis_identity: String,
    pub jobs: usize,
    /// Collects what each value node resolved to, for the canvas to show against the
    /// image being previewed. It also visits nodes no output depends on, which a batch
    /// run has no reason to evaluate but the canvas still displays.
    pub capture_values: bool,
}
impl Default for RunOptions {
    fn default() -> Self {
        Self {
            named_paths: BTreeMap::new(),
            named_files: BTreeMap::new(),
            measured_as: BTreeMap::new(),
            overwrite: false,
            cancelled: Arc::new(AtomicBool::new(false)),
            analysis_identity: String::new(),
            // One pipeline per core, and the host gives each an equal share of the
            // threads. Half the cores with a single thread each, which is what this was,
            // left three quarters of a machine idle on a batch.
            jobs: std::thread::available_parallelism().map_or(1, |count| count.get()),
            capture_values: false,
        }
    }
}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BatchResult {
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub outputs: Vec<PathBuf>,
    /// What each value node resolved to on the last file, when `capture_values` asked
    /// for it. Never serialised: the command line reports counts and paths.
    #[serde(skip)]
    pub values: BTreeMap<String, Context>,
    /// The nodes that stopped the stream on the last file, so a preview can say why it
    /// has no image to show rather than reporting a failure. Also `capture_values` only.
    #[serde(skip)]
    pub stopped: Vec<String>,
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
    // Electron uses Intl.Collator with sensitivity `base`. Folding the Latin
    // characters accepted in Windows filenames keeps its case/accent-insensitive
    // behaviour while leaving stable-sort ties in import order.
    fn fold_name(path: &Path) -> String {
        let mut folded = String::new();
        for c in filename(path).to_lowercase().chars() {
            let replacement = match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
                'æ' => "ae",
                'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
                'ď' | 'đ' => "d",
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
                'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
                'ĥ' | 'ħ' => "h",
                'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
                'ĵ' => "j",
                'ķ' => "k",
                'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
                'ñ' | 'ń' | 'ņ' | 'ň' => "n",
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
                'œ' => "oe",
                'ŕ' | 'ŗ' | 'ř' => "r",
                'ś' | 'ŝ' | 'ş' | 'š' | 'ß' => "s",
                'ţ' | 'ť' | 'ŧ' => "t",
                'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
                'ŵ' => "w",
                'ý' | 'ÿ' | 'ŷ' => "y",
                'ź' | 'ż' | 'ž' => "z",
                _ => {
                    folded.push(c);
                    continue;
                }
            };
            folded.push_str(replacement);
        }
        folded
    }
    let a = fold_name(a);
    let b = fold_name(b);
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
/// Whether any of these definitions asks for something only a full inspection of the file
/// can answer. Everything else the metadata context carries comes from the header or the
/// directory entry, so it costs nothing, and asking for the rest means two more processes.
pub fn wants_heavy_metadata<'a>(
    definitions: impl Iterator<Item = &'a definition::CompiledDefinition>,
) -> bool {
    definitions.flat_map(|d| &d.metadata).any(|key| {
        key == "image.bit_depth" || key.starts_with("image.dpi_") || key.starts_with("image.exif.")
    })
}

/// Whether a node hands an image on. The ones that do not are the value nodes, whose
/// resolved numbers the canvas shows beside their ports.
fn produces_image(definition: &definition::CompiledDefinition) -> bool {
    definition
        .definition
        .outputs
        .iter()
        .any(|port| matches!(port.kind, PortType::Image | PortType::Mask))
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

fn write_output_log(
    outputs: &[PathBuf],
    duration: Duration,
    output_dir: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let Some(first) = outputs.first() else {
        return Ok(None);
    };
    let directory = output_dir
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_owned)
        .or_else(|| {
            first
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .map(Path::to_owned)
        })
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let path = directory.join(format!("outputlog_{timestamp}.log"));
    let seconds = duration.as_secs_f64();
    let average = seconds / outputs.len() as f64;
    let folder = output_dir
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Same as source".into());
    let mut content = format!(
        "Bite Output Log\nGenerated: Unix timestamp {timestamp}\n\nDuration:         {seconds:.2}s\nFiles output:     {}\nAvg per file:     {average:.2}s\nOutput folder:    {folder}\n\nFiles:\n",
        outputs.len()
    );
    for output in outputs {
        let displayed = if output_dir.is_some() {
            output
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            output.display().to_string()
        };
        content.push_str(&displayed);
        content.push('\n');
    }
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(Some(path))
}

/// Resolves every file of `work`, over whatever workers the host offers, in file order.
///
/// A host with no workers resolves them one after another on itself, so the order a plan
/// records and the order a run reports are the same either way. Workers pull from a shared
/// queue rather than taking a fixed slice, because files differ wildly in cost: one large
/// image in a fixed slice would leave its worker running long after the others had
/// finished.
fn scatter(
    host: &mut dyn ImageHost,
    options: &RunOptions,
    work: &[(PathBuf, Option<String>, BTreeMap<String, PathBuf>)],
    resolve: &(dyn Fn(&mut dyn ImageHost, usize) -> ItemOutcome + Sync),
) -> Vec<ItemOutcome> {
    let cancelled = || options.cancelled.load(Ordering::Relaxed);
    let mut workers = if work.len() > 1 {
        host.workers(options.jobs)
    } else {
        Vec::new()
    };
    if workers.is_empty() {
        return (0..work.len())
            .map(|index| {
                if cancelled() {
                    Err("Cancelled".into())
                } else {
                    resolve(host, index)
                }
            })
            .collect();
    }
    workers.truncate(work.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ItemOutcome>>> = (0..work.len())
        .map(|_| std::sync::Mutex::new(None))
        .collect();
    std::thread::scope(|scope| {
        for worker in &mut workers {
            let next = &next;
            let results = &results;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= results.len() {
                    break;
                }
                let outcome = if cancelled() {
                    Err("Cancelled".into())
                } else {
                    resolve(worker.as_mut(), index)
                };
                *results[index].lock().unwrap() = Some(outcome);
            });
        }
    });
    results
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap()
                .unwrap_or_else(|| Err("Cancelled".into()))
        })
        .collect()
}
/// The channels Split Channels offers, in the order `-separate` hands them back.
const CHANNELS: [&str; 4] = ["Red", "Green", "Blue", "Alpha"];
/// Splits a stream that ends by taking a single channel into the channel and what it was
/// taken from. Any other stream has no channel to share, and comes back as nothing.
fn channel_of(args: &[String]) -> Option<(usize, &[String])> {
    let [rest @ .., flag, name, separate, restore] = args else {
        return None;
    };
    if (flag.as_str(), separate.as_str(), restore.as_str()) != ("-channel", "-separate", "+channel")
    {
        return None;
    }
    CHANNELS
        .iter()
        .position(|channel| channel == name)
        .map(|index| (index, rest))
}
/// The mean of one stream, measuring every channel of an image at once.
///
/// Taking the mean of red, then green, then blue - which is what a normal-map check does -
/// reads and decodes the same file three times over. Separating the channels in one
/// command reports all of their means together, so the file is read once and the other two
/// are already known by the time they are asked for.
///
/// Anything that is not a plain channel of an image, and any channel the image turns out
/// not to have, is measured on its own as before.
fn mean_of(
    host: &mut dyn ImageHost,
    measured: &mut BTreeMap<Vec<String>, Vec<f64>>,
    args: Vec<String>,
) -> Result<f64, String> {
    if let Some((channel, image)) = channel_of(&args) {
        if !measured.contains_key(image) {
            let mut probe = image.to_vec();
            probe.extend(s(&["-separate", "-format", "%[fx:mean] ", "info:"]));
            let means = host
                .capture(&probe)?
                .split_whitespace()
                .map(|mean| mean.parse().unwrap_or(f64::NAN))
                .collect();
            measured.insert(image.to_vec(), means);
        }
        if let Some(mean) = measured[image].get(channel).filter(|mean| !mean.is_nan()) {
            return Ok(*mean);
        }
    }
    let mut args = args;
    args.extend(s(&["-format", "%[fx:mean]", "info:"]));
    host.capture(&args)?
        .trim()
        .parse()
        .map_err(|error: std::num::ParseFloatError| error.to_string())
}
/// Everything resolving one file needs that is the same for every file of an output.
struct ItemContext<'a> {
    g: &'a Graph,
    registry: &'a Registry,
    options: &'a RunOptions,
    plan: &'a crate::plan::Plan,
    contributors: &'a BTreeSet<String>,
    input_id: &'a String,
    output: &'a GraphNode,
    raw_output: &'a Context,
    heavy: bool,
}
impl ItemResult {
    fn new(output: ItemOutput, values: BTreeMap<String, Context>, stopped: Vec<String>) -> Self {
        Self {
            output,
            values,
            stopped,
        }
    }
}
/// Runs one input file through the graph and reports what it produced for this output.
///
/// This is the whole of a batch's per-file work, and the only part of a run that spawns
/// analysis processes. It reads nothing the other files write, which is what lets a host
/// resolve several at once.
fn resolve_item(
    host: &mut dyn ImageHost,
    context: &ItemContext<'_>,
    index: usize,
    file: &Path,
    set_name: &Option<String>,
    set_inputs: &BTreeMap<String, PathBuf>,
) -> ItemOutcome {
    let &ItemContext {
        g,
        registry,
        options,
        plan,
        contributors,
        input_id,
        output,
        raw_output,
        heavy,
    } = context;
    let mut values = BTreeMap::new();
    let mut stopped = Vec::new();
    let mut channel_means = BTreeMap::new();
    // What the chain runs over and what it measures are the same file in a batch; a
    // preview renders a thumbnail and measures the original it was made from.
    let measured = options.measured_as.get(file).map_or(file, PathBuf::as_path);
    let metadata = host.metadata(measured, heavy)?;
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
        // A preview evaluates every node, because a value node hanging off a
        // branch still shows its number even when no output reads it.
        if (!contributors.contains(id) && !options.capture_values) || id == input_id {
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
                            CompiledArg::resolve_all(args, &params).map_err(|e| e.to_string())?,
                        );
                    }
                }
            }
            CompiledImplementation::Native(executor) => match executor.as_str() {
                "gate" => {
                    if enabled && !bool_param(&params, "condition", true) {
                        outgoing = None;
                        if options.capture_values {
                            stopped.push(id.clone());
                        }
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
                                CompiledArg::resolve_all(args, &p).map_err(|e| e.to_string())?,
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
                        if let Some(edge) = g
                            .edges
                            .iter()
                            .find(|e| e.target == *id && e.target_handle == format!("in:{name}"))
                        {
                            if let Some(param) = edge.source_handle.strip_prefix("param:") {
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
                            args.extend(s(&["-evaluate", "set", "0%", "-colorspace", "Gray"]));
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
                        let value = mean_of(host, &mut channel_means, image.args)?;
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
        if options.capture_values && !produces_image(def) {
            values.insert(id.clone(), params.clone());
        }
        resolved.insert(id.clone(), params);
    }
    let output_params = resolved.get(&output.id).unwrap_or(raw_output);
    let image = input_stream(output, g, &streams, "input");
    match output.kind {
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            let Some(image) = image else {
                return Ok(ItemResult::new(ItemOutput::Skipped, values, stopped));
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
                if let Some(ParamValue::Structured(StructuredParam::RenameBlocks { blocks })) =
                    node.data.params.get("blocks")
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
            return Ok(ItemResult::new(
                ItemOutput::Image {
                    name,
                    args: image.args,
                    format: image.format,
                },
                values,
                stopped,
            ));
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            if image.is_none() || !bool_param(output_params, "_txo_condition", true) {
                return Ok(ItemResult::new(ItemOutput::Skipped, values, stopped));
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
            return Ok(ItemResult::new(
                ItemOutput::Text(fields.join(&sep)),
                values,
                stopped,
            ));
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
            return Ok(ItemResult::new(
                ItemOutput::Atlas(output_params.clone()),
                values,
                stopped,
            ));
        }
        _ => {}
    }
    Ok(ItemResult::new(ItemOutput::None, values, stopped))
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
        let files = match options.named_files.get(&input_flag) {
            Some(listed) if !listed.is_empty() => {
                let mut listed = listed.clone();
                listed.sort();
                listed.dedup();
                listed
            }
            _ => {
                let dir = options
                    .named_paths
                    .get(&input_flag)
                    .filter(|path| !path.as_os_str().is_empty())
                    .ok_or_else(|| {
                        let flag = if input_flag.is_empty() {
                            "input"
                        } else {
                            &input_flag
                        };
                        let node = output
                            .data
                            .params
                            .get("cliName")
                            .and_then(definition::to_value)
                            .map(|value| value.text())
                            .unwrap_or_default();
                        format!(
                            "Missing required flag: --{flag}\n  \
                             Needed to run output node \"{node}\".\n  \
                             Export a CLI script from Bite to see all required flags."
                        )
                    })?;
                images(dir)?
            }
        };
        let raw_output: Context = output
            .data
            .params
            .iter()
            .filter_map(|(k, v)| definition::to_value(v).map(|v| (k.clone(), v)))
            .collect();
        let flag = text(&raw_output, "cliName", "");
        // The output's own skip/overwrite drop-down decides whether an existing file is
        // replaced, as the Electron run button's did. `--overwrite` on the command line
        // forces it for every output, which is what a repeated benchmark run wants.
        let overwrite = options.overwrite || text(&raw_output, "overwrite", "skip") == "overwrite";
        // Emitting still stages beside the destination, but a noclobber persist would refuse
        // the replacement this output just asked for, so the effective flag travels with it.
        let emit_options = RunOptions {
            overwrite,
            ..options.clone()
        };
        // A host that has no path for an endpoint leaves it out or empty; either way the
        // output's own parameters say where it goes.
        let dest = options
            .named_paths
            .get(&flag)
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .or_else(|| match output.kind {
                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                    // A wired Folder Path names the destination whatever the node's own
                    // drop-down says, which is the order the Electron run button resolved
                    // them in. Only a path the host passed for this flag outranks it.
                    graph::connected_folder(g, &output.id)
                        .map(PathBuf::from)
                        .or_else(|| {
                            (text(&raw_output, "outputPath", "source") == "custom")
                                .then(|| PathBuf::from(text(&raw_output, "customPath", "")))
                        })
                }
                NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                    Some(PathBuf::from(text(&raw_output, "outputPath", "")))
                }
                _ => Some(PathBuf::from(text(&raw_output, "flipbookOutputPath", ""))),
            });
        if output.kind != NodeKind::Builtin(BuiltinNodeKind::ImageOutput)
            && dest.as_ref().is_some_and(|p| p.exists())
            && !overwrite
        {
            result.skipped += files.len();
            continue;
        }
        let output_started = Instant::now();
        let first_output = result.outputs.len();
        let contributors: BTreeSet<_> = output_plan.contributors.iter().cloned().collect();
        // A preview evaluates the whole graph, so what any of it asks about the file has
        // to be read, not only what this output's own chain needs.
        let heavy = if options.capture_values {
            wants_heavy_metadata(
                g.nodes
                    .iter()
                    .filter_map(|n| registry.nodes.get(&n.data.definition_id)),
            )
        } else {
            wants_heavy_metadata(
                output_plan
                    .operations
                    .iter()
                    .filter_map(|o| registry.nodes.get(&o.definition)),
            )
        };
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
        let mut pending = Vec::new();
        // Every file resolves on its own: it reads its own metadata and runs its own
        // analysis, and nothing it does is visible to another. Handing the whole set to
        // the host lets one that can run processes concurrently do so, which is where a
        // chain with a mean or a property spends nearly all of its time. Placement, the
        // collision suffix and the counters stay here, applied in file order, so the run
        // lands exactly where a serial one would have.
        let context = ItemContext {
            g,
            registry,
            options,
            plan: &plan,
            contributors: &contributors,
            input_id,
            output,
            raw_output: &raw_output,
            heavy,
        };
        let resolved = scatter(host, options, &work, &|host, index| {
            let (file, set_name, set_inputs) = &work[index];
            resolve_item(host, &context, index, file, set_name, set_inputs)
        });
        for (index, ((file, _, _), item)) in work.iter().zip(resolved).enumerate() {
            let item = match item {
                Ok(item) => item,
                Err(error) => {
                    host.image_error(file, &output.id, &error);
                    if options.cancelled.load(Ordering::Relaxed)
                        || !host.continue_after_image_error()
                    {
                        return Err(error);
                    }
                    result.failed += 1;
                    result.errors.push(format!(
                        "{} (output {}): {error}",
                        file.display(),
                        output.id
                    ));
                    progress(index + 1, work.len(), file);
                    continue;
                }
            };
            result.values.extend(item.values);
            result.stopped.extend(item.stopped);
            let mut queued = false;
            match item.output {
                ItemOutput::None => {}
                ItemOutput::Skipped => result.skipped += 1,
                ItemOutput::Image { name, args, format } => {
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
                    if out.exists() && !overwrite {
                        result.skipped += 1;
                    } else {
                        let operation = if args.len() == 1 && format.is_none() {
                            OutputOperation::Copy {
                                source: args[0].clone().into(),
                                output: out.clone(),
                            }
                        } else {
                            OutputOperation::Image {
                                args,
                                format,
                                output: out.clone(),
                            }
                        };
                        pending.push((index, file.clone(), out, operation));
                        queued = true;
                    }
                }
                ItemOutput::Text(line) => {
                    report.push(line);
                    result.processed += 1;
                }
                ItemOutput::Atlas(params) => {
                    atlas.push(file.clone());
                    atlas_params = params;
                }
            }
            if !queued {
                progress(index + 1, work.len(), file);
            }
        }
        if !pending.is_empty() {
            let operations = pending
                .iter()
                .map(|(_, _, _, operation)| operation.clone())
                .collect();
            let emitted = host.emit_many(operations, &emit_options, options.jobs);
            for ((index, file, out, _), emitted) in pending.into_iter().zip(emitted) {
                match emitted {
                    Ok(()) => {
                        result.processed += 1;
                        result.outputs.push(out);
                    }
                    Err(error) => {
                        host.image_error(&file, &output.id, &error);
                        if options.cancelled.load(Ordering::Relaxed)
                            || !host.continue_after_image_error()
                        {
                            return Err(error);
                        }
                        result.failed += 1;
                        result.errors.push(format!(
                            "{} (output {}): {error}",
                            file.display(),
                            output.id
                        ));
                    }
                }
                progress(index + 1, work.len(), &file);
            }
        }
        if output.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput) && !report.is_empty() {
            if !report.iter().any(|line| !line.trim().is_empty()) {
                return Err("All values resolved to empty - file not written.".into());
            }
            let out = dest.clone().ok_or("text output path missing")?;
            if out.as_os_str().is_empty() {
                return Err("text output path empty".into());
            }
            host.emit(
                OutputOperation::Text {
                    output: out.clone(),
                    contents: report.join("\n") + "\n",
                },
                &emit_options,
            )?;
            result.outputs.push(out);
        }
        // A Gate that shut on every file leaves nothing to tile, and `montage` with no
        // pictures fails with a message about its own arguments rather than reporting a
        // run in which nothing reached the output.
        if output.kind == NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) && !atlas.is_empty() {
            let out = dest.clone().ok_or("flipbook output path missing")?;
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
                &emit_options,
            )?;
            result.processed += atlas.len();
            result.outputs.push(out);
        }
        if bool_param(&raw_output, "generateLog", false) {
            let new_outputs = &result.outputs[first_output..];
            let log_dir = match output.kind {
                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                    dest.as_deref().filter(|path| !path.as_os_str().is_empty())
                }
                _ => dest
                    .as_deref()
                    .and_then(Path::parent)
                    .filter(|path| !path.as_os_str().is_empty()),
            };
            if let Err(error) = write_output_log(new_outputs, output_started.elapsed(), log_dir) {
                result.errors.push(format!("Output log: {error}"));
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers a separation of every channel with one mean each, and a single channel
    /// with its own, so what a measurement cost can be counted.
    #[derive(Default)]
    struct MeanHost {
        captures: Vec<Vec<String>>,
    }
    impl ImageHost for MeanHost {
        fn run(&mut self, _: &[String]) -> Result<(), String> {
            Ok(())
        }
        fn metadata(&mut self, _: &Path, _: bool) -> Result<Context, String> {
            Ok(Context::new())
        }
        fn capture(&mut self, args: &[String]) -> Result<String, String> {
            self.captures.push(args.to_vec());
            let has = |arg: &str| args.iter().any(|item| item == arg);
            Ok(match (has("-separate"), has("-channel")) {
                // Every channel of a three channel image, then one named channel, then
                // an image that was never separated at all.
                (true, false) => "0.1 0.2 0.3 ",
                (true, true) => "0.25",
                _ => "0.5",
            }
            .into())
        }
    }

    #[test]
    fn channel_means_of_one_image_are_measured_in_a_single_pass() {
        let mut host = MeanHost::default();
        let mut measured = BTreeMap::new();
        let channel = |name: &str| s(&["image.png", "-channel", name, "-separate", "+channel"]);
        for (name, expected) in [("Red", 0.1), ("Green", 0.2), ("Blue", 0.3)] {
            let mean = mean_of(&mut host, &mut measured, channel(name)).unwrap();
            assert_eq!(mean, expected, "{name}");
        }
        assert_eq!(
            host.captures,
            [s(&[
                "image.png",
                "-separate",
                "-format",
                "%[fx:mean] ",
                "info:"
            ])],
            "three channels of one image are one read, not three"
        );

        // The image has no fourth channel, so alpha is the one that still costs a read.
        assert_eq!(
            mean_of(&mut host, &mut measured, channel("Alpha")).unwrap(),
            0.25
        );
        assert_eq!(host.captures.len(), 2);
        assert!(host.captures[1].contains(&"Alpha".to_string()));

        // Anything that is not a plain channel is measured as it always was.
        let mut host = MeanHost::default();
        let blurred = s(&["image.png", "-blur", "0x1"]);
        assert_eq!(
            mean_of(&mut host, &mut BTreeMap::new(), blurred.clone()).unwrap(),
            0.5
        );
        assert_eq!(host.captures[0][..3], blurred[..3]);
        assert!(!host.captures[0].contains(&"-separate".to_string()));
    }

    #[test]
    fn natural_sort_matches_base_sensitive_numeric_collation() {
        let mut names =
            ["Ábaco2.png", "abaco10.png", "äbaco1.png", "Éclair3.png"].map(PathBuf::from);
        names.sort_by(|a, b| natural_name_cmp(a, b));
        assert_eq!(
            names.map(|p| p.to_string_lossy().into_owned()),
            ["äbaco1.png", "Ábaco2.png", "abaco10.png", "Éclair3.png"]
        );
    }

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

    #[test]
    fn output_log_lists_generated_files_next_to_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let outputs = [dir.path().join("a.png"), dir.path().join("b.png")];
        let log = write_output_log(&outputs, Duration::from_millis(2500), Some(dir.path()))
            .unwrap()
            .unwrap();
        let content = fs::read_to_string(log).unwrap();
        assert!(content.contains("Duration:         2.50s"));
        assert!(content.contains("Files output:     2"));
        assert!(content.contains("a.png\nb.png"));
    }
}
