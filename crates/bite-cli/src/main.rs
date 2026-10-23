use bite_schema::{
    BuiltinNodeKind, FormatDefinition, Graph, GraphNode, NodeDefinition, NodeKind, ParamValue,
    Workflow,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

fn run() -> Result<(), Fail> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("bite run <workflow.bite> [--<cliName> <path>] [--overwrite] [--jobs N] [--run-report <json>]\n  --overwrite  Overwrite existing output files (default: skip)\n  --jobs N     Bounded ImageMagick worker count (safe default: half the CPUs, max 8)\n  --run-report <json>  Write the counts, the files written, and the ones that failed and why\n  Exits 0 when every file worked, 1 when one failed or the run stopped, and 2 when the\n  command line names something the workflow lacks or leaves out an input's folder.\nbite describe <workflow.bite>\n  The workflow's inputs and outputs as JSON: each one's CLI name and what its path names.\nbite plan <workflow.bite> [--<cliName> <path>] [--overwrite] [--facts <json>]\n  Reports complete=false when image analysis facts are required.\nbite observe <workflow.bite> [--<cliName> <path>] --facts-out <json>\n  Runs only required analysis and writes provenance-bound planning facts.\nbite bench-import <folder> [--recursive] [--size 256] [--jobs N]\n  Measures scan plus cold/warm native thumbnail import.\nbite bench-workflow <workflow.bite> --overwrite [--iterations 2] [run flags]\n  Measures repeated native workflow throughput and process/cache counts.\nbite bench-cancel <workflow.bite> [--after-ms 250] [run flags]\n  Measures cooperative cancellation latency during a native workflow.\nbite inspect|validate <workflow.bite>\nbite validate-node <file>\nbite validate-format <file>\nbite validate-workflow <file>\nbite expr --params <json> <expression>\nbite schemas <directory>\nbite nodes|formats|version|--version");
        return Ok(());
    }
    if args[0] == "bench-cancel" {
        if args.len() < 2 {
            return Err(
                "usage: bite bench-cancel <workflow.bite> [--after-ms 250] [run flags]".into(),
            );
        }
        let mut options = bite_core::execution::RunOptions::default();
        let mut after_ms = 250u64;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--overwrite" => options.overwrite = true,
                "--after-ms" | "--jobs" => {
                    let flag = args[i].clone();
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    if flag == "--after-ms" {
                        after_ms = value.parse().map_err(|_| "invalid --after-ms")?;
                        if after_ms > 60_000 {
                            return Err("--after-ms must be at most 60000".into());
                        }
                    } else {
                        options.jobs = value.parse().map_err(|_| "invalid --jobs")?;
                        if options.jobs == 0 || options.jobs > 256 {
                            return Err("--jobs must be between 1 and 256".into());
                        }
                    }
                }
                flag if flag.starts_with("--") => {
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{flag} requires a path"))?;
                    let path = std::path::PathBuf::from(value);
                    options.named_paths.insert(
                        flag.trim_start_matches("--").into(),
                        if path.is_absolute() {
                            path
                        } else {
                            env::current_dir()
                                .map_err(|error| error.to_string())?
                                .join(path)
                        },
                    );
                }
                value => {
                    return Err(Fail::Usage(format!(
                        "Unexpected bench-cancel argument: {value}"
                    )))
                }
            }
            i += 1;
        }
        let registry = load_registry()?;
        let workflow = bite_core::workflow::load(
            &fs::read_to_string(&args[1]).map_err(|error| error.to_string())?,
            &registry,
        )?;
        let triggered = std::sync::Arc::new(std::sync::Mutex::new(None));
        let trigger_time = triggered.clone();
        let cancelled = options.cancelled.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(after_ms));
            *trigger_time.lock().unwrap() = Some(Instant::now());
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
        let result = bite_core::execution::run_workflow(
            &workflow.workflow.graph,
            &registry,
            &mut host,
            &options,
            &mut |_| {},
        );
        trigger.join().map_err(|_| "cancellation trigger failed")?;
        let latency_ms = triggered
            .lock()
            .unwrap()
            .as_ref()
            .map(Instant::elapsed)
            .unwrap_or_default()
            .as_secs_f64()
            * 1000.0;
        if !result.is_err_and(|error| error == "Cancelled") {
            return Err("workflow did not return the expected Cancelled result".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "rust",
                "after_ms": after_ms,
                "cancellation_latency_ms": latency_ms,
                "jobs": options.jobs,
                "imagemagick_processes_started": host.processes(),
                "peak_child_memory_bytes": host.peak_child_memory_bytes(),
            }))
            .map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if args[0] == "bench-workflow" {
        if args.len() < 2 {
            return Err("usage: bite bench-workflow <workflow.bite> --overwrite [--iterations 2] [run flags]".into());
        }
        let mut options = bite_core::execution::RunOptions::default();
        let mut iterations = 2usize;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--overwrite" => options.overwrite = true,
                "--iterations" | "--jobs" => {
                    let flag = args[i].clone();
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    if flag == "--iterations" {
                        iterations = value.parse().map_err(|_| "invalid --iterations")?;
                        if iterations == 0 || iterations > 1000 {
                            return Err("--iterations must be between 1 and 1000".into());
                        }
                    } else {
                        options.jobs = value.parse().map_err(|_| "invalid --jobs")?;
                        if options.jobs == 0 || options.jobs > 256 {
                            return Err("--jobs must be between 1 and 256".into());
                        }
                    }
                }
                flag if flag.starts_with("--") => {
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{flag} requires a path"))?;
                    let path = std::path::PathBuf::from(value);
                    options.named_paths.insert(
                        flag.trim_start_matches("--").into(),
                        if path.is_absolute() {
                            path
                        } else {
                            env::current_dir()
                                .map_err(|error| error.to_string())?
                                .join(path)
                        },
                    );
                }
                value => {
                    return Err(Fail::Usage(format!(
                        "Unexpected bench-workflow argument: {value}"
                    )))
                }
            }
            i += 1;
        }
        if iterations > 1 && !options.overwrite {
            return Err("bench-workflow needs --overwrite for repeated comparable runs".into());
        }
        let registry = load_registry()?;
        let workflow = bite_core::workflow::load(
            &fs::read_to_string(&args[1]).map_err(|error| error.to_string())?,
            &registry,
        )?;
        let cancelled = options.cancelled.clone();
        ctrlc::set_handler(move || cancelled.store(true, std::sync::atomic::Ordering::Relaxed))
            .map_err(|error| format!("Cannot install cancellation handler: {error}"))?;
        let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
        let mut measurements = Vec::new();
        for iteration in 0..iterations {
            let processes = host.processes();
            let hits = host.metadata_cache_stats().hits;
            let misses = host.metadata_cache_stats().misses;
            let capture_hits = host.capture_cache_stats().hits;
            let capture_misses = host.capture_cache_stats().misses;
            host.reset_peak_child_memory();
            let started = Instant::now();
            let result = bite_core::execution::run_workflow(
                &workflow.workflow.graph,
                &registry,
                &mut host,
                &options,
                &mut |_| {},
            )?;
            measurements.push(serde_json::json!({
                "iteration": iteration + 1,
                "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
                "processed": result.processed,
                "filtered": result.filtered,
                "skipped": result.skipped,
                "failed": result.failed,
                "imagemagick_processes": host.processes() - processes,
                "peak_child_memory_bytes": host.peak_child_memory_bytes(),
                "metadata_cache_hits": host.metadata_cache_stats().hits - hits,
                "metadata_cache_misses": host.metadata_cache_stats().misses - misses,
                "capture_cache_hits": host.capture_cache_stats().hits - capture_hits,
                "capture_cache_misses": host.capture_cache_stats().misses - capture_misses,
            }));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "rust",
                "workflow": args[1],
                "jobs": options.jobs,
                "runs": measurements,
            }))
            .map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if args[0] == "bench-import" {
        if args.len() < 2 {
            return Err(
                "usage: bite bench-import <folder> [--recursive] [--size 256] [--jobs N]".into(),
            );
        }
        let root = std::path::PathBuf::from(&args[1]);
        let root = if root.is_absolute() {
            root
        } else {
            env::current_dir()
                .map_err(|error| error.to_string())?
                .join(root)
        };
        let mut recursive = false;
        let mut size = 256u32;
        let mut jobs = bite_imagemagick::import::default_jobs();
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--recursive" => recursive = true,
                "--size" | "--jobs" => {
                    let flag = args[i].clone();
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    if flag == "--size" {
                        size = value.parse().map_err(|_| "invalid --size")?;
                    } else {
                        jobs = value.parse().map_err(|_| "invalid --jobs")?;
                        if jobs == 0 || jobs > 256 {
                            return Err("--jobs must be between 1 and 256".into());
                        }
                    }
                }
                flag => return Err(Fail::Usage(format!("Unknown bench-import option: {flag}"))),
            }
            i += 1;
        }
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = cancelled.clone();
        ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Relaxed))
            .map_err(|error| format!("Cannot install cancellation handler: {error}"))?;
        let started = Instant::now();
        let paths = bite_imagemagick::import::scan_folder(&root, recursive, jobs, &cancelled)?;
        let scan_ms = started.elapsed().as_secs_f64() * 1000.0;
        if paths.is_empty() {
            return Err(format!("No images found in: {}", root.display()).into());
        }
        let cache_dir = env::current_dir()
            .map_err(|error| error.to_string())?
            .join("test-workflows/out")
            .join(format!("bench-import-cache-{}", std::process::id()));
        let mut cache = bite_imagemagick::import::ThumbnailCache::new(cache_dir.clone());
        cache.jobs = jobs;
        let mut host = bite_imagemagick::Magick::discover(cancelled);
        host.reset_peak_child_memory();
        let cold_started = Instant::now();
        let cold = cache.load_batch(&mut host, &paths, size)?;
        let cold_ms = cold_started.elapsed().as_secs_f64() * 1000.0;
        let cold_processes = host.processes();
        let cold_peak_child_memory = host.peak_child_memory_bytes();
        let after_cold = cache.stats;
        host.reset_peak_child_memory();
        let warm_started = Instant::now();
        let warm = cache.load_batch(&mut host, &paths, size)?;
        let warm_ms = warm_started.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "rust",
                "root": root,
                "images": paths.len(),
                "jobs": jobs,
                "thumbnail_size": size,
                "scan_ms": scan_ms,
                "cold": {
                    "elapsed_ms": cold_ms,
                    "images": cold.len(),
                    "processes": cold_processes,
                    "peak_child_memory_bytes": cold_peak_child_memory,
                    "disk_hits": after_cold.disk_hits,
                    "disk_misses": after_cold.disk_misses,
                },
                "warm": {
                    "elapsed_ms": warm_ms,
                    "images": warm.len(),
                    "processes": host.processes() - cold_processes,
                    "peak_child_memory_bytes": host.peak_child_memory_bytes(),
                    "memory_hits": cache.stats.memory_hits - after_cold.memory_hits,
                    "disk_hits": cache.stats.disk_hits - after_cold.disk_hits,
                },
                "cache_directory": cache_dir,
            }))
            .map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if args[0] == "run" || args[0] == "plan" || args[0] == "observe" {
        return workflow_command(&args);
    }
    if ["nodes", "formats"].contains(&args[0].as_str()) {
        let registry = load_registry()?;
        let ids = if args[0] == "nodes" {
            registry.nodes.keys().collect::<Vec<_>>()
        } else {
            registry.formats.keys().collect::<Vec<_>>()
        };
        for id in ids {
            println!("{id}");
        }
        return Ok(());
    }
    if args[0] == "version" || args[0] == "--version" || args[0] == "-V" {
        println!("bite {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args[0] == "describe" {
        if args.len() != 2 {
            return Err(Fail::Usage("usage: bite describe <workflow.bite>".into()));
        }
        let registry = load_registry()?;
        let loaded = bite_core::workflow::load(&read_workflow(&args[1])?, &registry)?;
        let endpoints = endpoints(&loaded.workflow.graph, &registry)?;
        let (inputs, outputs): (Vec<_>, Vec<_>) = endpoints.iter().partition(|e| e.kind == "input");
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "workflow": args[1],
                "inputs": inputs.iter().map(|e| serde_json::json!({
                    "name": e.name,
                    "label": e.label,
                    "required": e.required,
                })).collect::<Vec<_>>(),
                "outputs": outputs.iter().map(|e| serde_json::json!({
                    "name": e.name,
                    "label": e.label,
                    "kind": e.kind,
                    "takes": e.takes,
                    "path": e.path,
                })).collect::<Vec<_>>(),
            }))
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if ["inspect", "validate"].contains(&args[0].as_str()) {
        if args.len() != 2 {
            return Err(Fail::Usage(
                "usage: bite inspect|validate workflow.bite".into(),
            ));
        }
        let registry = load_registry()?;
        let loaded = bite_core::workflow::load(
            &fs::read_to_string(&args[1]).map_err(|e| e.to_string())?,
            &registry,
        )?;
        for warning in loaded.warnings {
            eprintln!("[Bite] {warning}");
        }
        if args[0] == "inspect" {
            println!(
                "Workflow schema: {}\nNodes: {}\nEdges: {}\nExecution order:",
                if loaded.migrated {
                    "1 -> migrated to 2"
                } else {
                    "2"
                },
                loaded.workflow.graph.nodes.len(),
                loaded.workflow.graph.edges.len()
            );
            for id in bite_core::graph::topo_sort(&loaded.workflow.graph)? {
                println!("  {id}");
            }
        } else {
            println!("OK: {}", args[1]);
        }
        return Ok(());
    }
    if args[0] == "expr" {
        if args.len() != 4 || args[1] != "--params" {
            return Err(Fail::Usage(
                "usage: bite expr --params '{\"width\":1024}' 'width / 2'".into(),
            ));
        }
        let ctx: bite_expr::Context = serde_json::from_str(&args[2]).map_err(|e| e.to_string())?;
        let types = ctx.iter().map(|(k, v)| (k.clone(), v.kind())).collect();
        let expr = bite_expr::Expression::compile(&args[3], &types).map_err(|e| e.to_string())?;
        println!("{}", expr.evaluate(&ctx).map_err(|e| e.to_string())?.text());
        return Ok(());
    }
    if ![
        "schemas",
        "validate-node",
        "validate-format",
        "validate-workflow",
    ]
    .contains(&args[0].as_str())
    {
        return Err(Fail::Usage(format!(
            "Unknown command: {:?}. Run \"bite --help\" for usage.",
            args[0]
        )));
    }
    if args.len() != 2 {
        return Err(Fail::Usage(
            "expected a command and one path; use --help".into(),
        ));
    }
    if args[0] == "schemas" {
        fs::create_dir_all(&args[1]).map_err(|e| e.to_string())?;
        for (name, schema) in bite_schema::json_schemas() {
            fs::write(
                std::path::Path::new(&args[1]).join(name),
                serde_json::to_string_pretty(&schema).unwrap() + "\n",
            )
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let json = fs::read_to_string(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
    let (label, result) = match args[0].as_str() {
        "validate-node" => {
            let d: NodeDefinition = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let label = d.id.clone();
            bite_expr::definition::CompiledDefinition::compile(d).map_err(|e| e.to_string())?;
            (label, Ok(()))
        }
        "validate-format" => {
            let d: FormatDefinition = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let r = d.validate();
            bite_expr::CompiledArg::compile_all(
                &d.args,
                &bite_expr::definition::parameter_types(&d.params),
            )
            .map_err(|e| e.to_string())?;
            (d.id, r)
        }
        "validate-workflow" => {
            let d: Workflow = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            (args[1].clone(), d.validate())
        }
        cmd => return Err(format!("unknown command {cmd:?}").into()),
    };
    result.map_err(|errors| errors.join("\n"))?;
    println!("OK: {label}");
    Ok(())
}
/// `run`, `plan` and `observe`: a workflow, a path for each of its inputs and outputs that
/// the command line names (`--<name> <path>`), and the options each command takes.
fn workflow_command(args: &[String]) -> Result<(), Fail> {
    let command = args[0].as_str();
    let Some(workflow_file) = args.get(1).filter(|arg| !arg.starts_with("--")) else {
        return Err(Fail::Usage(format!(
            "Missing workflow file argument.\nUsage: bite {command} <workflow.bite>"
        )));
    };
    let mut options = bite_core::execution::RunOptions::default();
    let planning = command == "plan";
    let observing = command == "observe";
    let mut facts_out = None;
    let mut facts_file = None;
    let mut report = None;
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--overwrite" => options.overwrite = true,
            "--jobs" => {
                options.jobs = value(args, &mut i, "a number")?
                    .parse()
                    .ok()
                    .filter(|jobs| (1..=256).contains(jobs))
                    .ok_or_else(|| Fail::Usage("--jobs must be between 1 and 256".into()))?;
            }
            "--facts-out" if observing => {
                facts_out = Some(PathBuf::from(value(args, &mut i, "a JSON file")?))
            }
            "--facts" if planning => facts_file = Some(value(args, &mut i, "a JSON file")?),
            "--run-report" if command == "run" => {
                report = Some(absolute(value(args, &mut i, "a JSON file")?)?)
            }
            _ => match flag.strip_prefix("--").filter(|name| !name.is_empty()) {
                Some(name) => {
                    let path = absolute(value(args, &mut i, "a path")?)?;
                    options.named_paths.insert(name.into(), path);
                }
                None => return Err(Fail::Usage(format!("Unexpected argument: {flag}"))),
            },
        }
        i += 1;
    }
    let cancelled = options.cancelled.clone();
    ctrlc::set_handler(move || cancelled.store(true, std::sync::atomic::Ordering::Relaxed))
        .map_err(|e| format!("Cannot install cancellation handler: {e}"))?;
    if command == "run" {
        let outcome = run_batch(workflow_file, &options);
        if let Some(path) = &report {
            write_report(path, workflow_file, &outcome)?;
        }
        let result = outcome?;
        if result.failed > 0 {
            let total = result.processed + result.filtered + result.skipped + result.failed;
            return Err(format!("{} of {total} failed.", result.failed).into());
        }
        return Ok(());
    }
    let mut facts = bite_core::plan::PlanningFacts::default();
    if let Some(path) = facts_file {
        facts = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid planning facts: {e}"))?;
    }
    let (registry, loaded) = load_checked(workflow_file, &options)?;
    {
        let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
        if observing {
            use bite_core::{execution::ImageHost, plan::AnalysisDependency};
            let output: std::path::PathBuf =
                facts_out.ok_or("observe requires --facts-out <json>")?;
            options.analysis_identity = host.analysis_identity()?;
            for _ in 0..256 {
                if !facts.metadata.is_empty() || !facts.captures.is_empty() {
                    facts.seal(
                        &loaded.workflow.graph,
                        &registry,
                        &options.analysis_identity,
                    )?;
                }
                let plan = {
                    let mut metadata = |path: &std::path::Path, heavy| {
                        if heavy {
                            Err(format!("Metadata analysis required for {}", path.display()))
                        } else {
                            host.metadata(path, false)
                        }
                    };
                    bite_core::plan::concrete(
                        &loaded.workflow.graph,
                        &registry,
                        &options,
                        &facts,
                        &mut metadata,
                    )?
                };
                if plan.complete {
                    facts.seal(
                        &loaded.workflow.graph,
                        &registry,
                        &options.analysis_identity,
                    )?;
                    if let Some(parent) = output.parent() {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    fs::write(
                        &output,
                        serde_json::to_string_pretty(&facts).map_err(|error| error.to_string())?
                            + "\n",
                    )
                    .map_err(|error| error.to_string())?;
                    println!("Observed planning facts: {}", output.display());
                    return Ok(());
                }
                let mut added = 0;
                for dependency in plan.dependencies {
                    match dependency {
                        AnalysisDependency::Capture { args, .. } => {
                            let value = host.capture(&args)?;
                            facts
                                .captures
                                .push(bite_core::plan::CaptureFact { args, value });
                            added += 1;
                        }
                        AnalysisDependency::Metadata { path, heavy, .. } => {
                            facts
                                .metadata
                                .insert(path.clone(), host.metadata(&path, heavy)?);
                            added += 1;
                        }
                    }
                }
                if added == 0 {
                    return Err("Observation made no progress resolving planning facts".into());
                }
            }
            return Err("Observation exceeded 256 dependency rounds".into());
        }
        use bite_core::execution::ImageHost;
        if !facts.metadata.is_empty() || !facts.captures.is_empty() {
            options.analysis_identity = host.analysis_identity()?;
        }
        let mut metadata = |path: &std::path::Path, heavy| {
            if heavy {
                Err(format!(
                    "Metadata analysis required for {}; supply metadata in --facts",
                    path.display()
                ))
            } else {
                host.metadata(path, false)
            }
        };
        let plan = bite_core::plan::concrete(
            &loaded.workflow.graph,
            &registry,
            &options,
            &facts,
            &mut metadata,
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}
/// Runs a workflow over the folders the command line gave it, printing each file as it
/// finishes, then what the run came to.
fn run_batch(
    workflow_file: &str,
    options: &bite_core::execution::RunOptions,
) -> Result<bite_core::execution::BatchResult, Fail> {
    let (registry, loaded) = load_checked(workflow_file, options)?;
    let mut host = bite_imagemagick::Magick::discover(options.cancelled.clone());
    let result = bite_core::execution::run_workflow(
        &loaded.workflow.graph,
        &registry,
        &mut host,
        options,
        &mut |progress| {
            if let Some(file) = progress.file {
                println!(
                    "[{}/{}] {}",
                    progress.completed,
                    progress.total,
                    file.display()
                );
            }
        },
    )?;
    println!(
        "Done: {} processed, {} filtered, {} skipped, {} failed",
        result.processed, result.filtered, result.skipped, result.failed
    );
    for error in &result.errors {
        eprintln!("[Bite] Failed to process {error}");
    }
    Ok(result)
}
/// Writes what `--run-report` asks for: the run's counts, the files it wrote, the ones that
/// failed and why, and the problem that stopped it when one did. It is written however the
/// run ends, so whoever asked for it always has it to read.
fn write_report(
    path: &Path,
    workflow_file: &str,
    outcome: &Result<bite_core::execution::BatchResult, Fail>,
) -> Result<(), Fail> {
    let nothing = bite_core::execution::BatchResult::default();
    let result = outcome.as_ref().unwrap_or(&nothing);
    let mut report = serde_json::json!({
        "bite": env!("CARGO_PKG_VERSION"),
        "workflow": absolute(workflow_file)?,
        "processed": result.processed,
        "filtered": result.filtered,
        "skipped": result.skipped,
        "failed": result.failed,
        "outputs": result.outputs,
        "failures": result.failures,
    });
    if let Err(fail) = outcome {
        report["problem"] = fail.message().into();
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())? + "\n";
    fs::write(path, text).map_err(|e| {
        Fail::Work(format!(
            "Cannot write the run report {}: {e}",
            path.display()
        ))
    })
}
/// The workflow, loaded, once the command line has been checked against it.
fn load_checked(
    workflow_file: &str,
    options: &bite_core::execution::RunOptions,
) -> Result<(bite_core::Registry, bite_core::workflow::LoadedWorkflow), Fail> {
    let registry = load_registry()?;
    let loaded = bite_core::workflow::load(&read_workflow(workflow_file)?, &registry)?;
    check_paths(
        &endpoints(&loaded.workflow.graph, &registry)?,
        &options.named_paths,
    )?;
    Ok((registry, loaded))
}
fn read_workflow(file: &str) -> Result<String, Fail> {
    fs::read_to_string(file).map_err(|_| Fail::Usage(format!("Cannot read workflow: {file}")))
}
/// The value after the option at `args[*i]`, moving `i` onto it. Another option, nothing,
/// or an empty value in its place is a command line that can't be followed: an empty path
/// would otherwise mean the current folder.
fn value<'a>(args: &'a [String], i: &mut usize, what: &str) -> Result<&'a str, Fail> {
    let flag = &args[*i];
    *i += 1;
    args.get(*i)
        .map(String::as_str)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .ok_or_else(|| Fail::Usage(format!("{flag} needs {what}")))
}
fn absolute(path: &str) -> Result<PathBuf, Fail> {
    let path = PathBuf::from(path);
    Ok(if path.is_absolute() {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?.join(path)
    })
}
/// An Input or output node, which a command line names with `--<name> <path>`.
struct Endpoint {
    /// The node's CLI name, exactly as a run looks it up. Empty when it has none, and then
    /// no command line can reach it.
    name: String,
    label: String,
    /// `input`, `image`, `text` or `flipbook`.
    kind: &'static str,
    /// What its path names: a `folder` (of images to read, or to write them to) or a `file`.
    takes: &'static str,
    /// Where an output writes when the command line doesn't say: the folder or file the
    /// workflow names, or nothing for an Image Output that writes beside each image it read.
    path: String,
    /// An Input some output reads from, which a run can't go without.
    required: bool,
}
/// The workflow's inputs, then its outputs, each in the order the graph holds them.
fn endpoints(graph: &Graph, registry: &bite_core::Registry) -> Result<Vec<Endpoint>, String> {
    let read: BTreeSet<String> = bite_core::plan::build(graph, registry)?
        .outputs
        .into_iter()
        .filter_map(|output| output.input)
        .collect();
    let text = |node: &GraphNode, name: &str| match node.data.params.get(name) {
        Some(ParamValue::String(value)) => value.clone(),
        _ => String::new(),
    };
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for node in &graph.nodes {
        let (kind, takes, path) = match node.kind {
            NodeKind::Builtin(BuiltinNodeKind::Input) => ("input", "folder", String::new()),
            NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => (
                "image",
                "folder",
                bite_core::graph::connected_folder(graph, &node.id)
                    .or_else(|| {
                        (text(node, "outputPath") == "custom").then(|| text(node, "customPath"))
                    })
                    .unwrap_or_default(),
            ),
            NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                ("text", "file", text(node, "outputPath"))
            }
            NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
                ("flipbook", "file", text(node, "flipbookOutputPath"))
            }
            _ => continue,
        };
        let endpoint = Endpoint {
            name: text(node, "cliName"),
            label: node.data.label.clone(),
            kind,
            takes,
            path,
            required: read.contains(&node.id),
        };
        if kind == "input" {
            inputs.push(endpoint);
        } else {
            outputs.push(endpoint);
        }
    }
    inputs.extend(outputs);
    Ok(inputs)
}
/// Refuses a path for something the workflow doesn't have, and a run without a folder one
/// of its outputs reads from. Either would otherwise go ahead on what the workflow itself
/// says, and an output whose name was mistyped writes beside the images it read.
fn check_paths(endpoints: &[Endpoint], given: &BTreeMap<String, PathBuf>) -> Result<(), Fail> {
    for name in given.keys() {
        if !endpoints.iter().any(|endpoint| endpoint.name == *name) {
            let known: Vec<String> = endpoints
                .iter()
                .filter(|endpoint| !endpoint.name.is_empty())
                .map(|endpoint| format!("--{}", endpoint.name))
                .collect();
            return Err(Fail::Usage(if known.is_empty() {
                format!("Unknown option --{name}. This workflow has no CLI names.")
            } else {
                format!(
                    "Unknown option --{name}. This workflow takes {}.",
                    known.join(", ")
                )
            }));
        }
    }
    if let Some(input) = endpoints
        .iter()
        .find(|endpoint| endpoint.required && !given.contains_key(&endpoint.name))
    {
        return Err(Fail::Usage(if input.name.is_empty() {
            format!(
                "The Input \"{}\" has no CLI name, so no command line can give it a folder. \
                 Name it in Bite's Inspector.",
                input.label
            )
        } else {
            format!(
                "Missing required option --{}, the folder of images for \"{}\".",
                input.name, input.label
            )
        }));
    }
    Ok(())
}
fn load_registry() -> Result<bite_core::Registry, String> {
    let mut roots = Vec::new();
    if let Some(root) = env::var_os("BITE_RESOURCES") {
        roots.push(std::path::PathBuf::from(root));
    }
    // Resolved first: on macOS `bite` on the PATH is a symlink into the app bundle, and
    // `current_exe` reports the link, beside which there are no definitions.
    if let Ok(exe) = env::current_exe().and_then(std::fs::canonicalize) {
        if let Some(parent) = exe.parent() {
            roots.push(parent.into());
            roots.push(parent.join("../Resources"));
            roots.push(parent.join("../.."));
        }
    }
    roots.push(env::current_dir().map_err(|e| e.to_string())?);
    for root in roots {
        let nodes = root.join("node-definitions");
        let formats = root.join("format-definitions");
        if nodes.is_dir() && formats.is_dir() {
            return bite_core::Registry::load(&nodes, &formats);
        }
    }
    Err("Cannot find node and format definitions; set BITE_RESOURCES".into())
}
/// Why a command stopped, which is also how the process ends: 2 when the command line can't
/// be followed (an unknown command or option, a value or an input's folder missing, a
/// workflow that isn't there), 1 when the work went wrong. A run ends 0 only when every
/// file worked.
enum Fail {
    Usage(String),
    Work(String),
}
impl Fail {
    fn message(&self) -> &str {
        match self {
            Fail::Usage(message) | Fail::Work(message) => message,
        }
    }
}
impl From<String> for Fail {
    fn from(message: String) -> Self {
        Fail::Work(message)
    }
}
impl From<&str> for Fail {
    fn from(message: &str) -> Self {
        Fail::Work(message.into())
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            eprintln!("[Bite] {}", fail.message());
            ExitCode::from(match fail {
                Fail::Usage(_) => 2,
                Fail::Work(_) => 1,
            })
        }
    }
}
