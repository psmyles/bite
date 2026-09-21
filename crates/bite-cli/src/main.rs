use bite_schema::{FormatDefinition, NodeDefinition, Workflow};
use std::{env, fs, process::ExitCode, time::Instant};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("bite run <workflow.bite> [--<cliName> <path>] [--overwrite] [--jobs N]\n  --overwrite  Overwrite existing output files (default: skip)\n  --jobs N     Bounded ImageMagick worker count (safe default: half the CPUs, max 8)\nbite plan <workflow.bite> [--<cliName> <path>] [--overwrite] [--facts <json>]\n  Reports complete=false when image analysis facts are required.\nbite observe <workflow.bite> [--<cliName> <path>] --facts-out <json>\n  Runs only required analysis and writes provenance-bound planning facts.\nbite bench-import <folder> [--recursive] [--size 256] [--jobs N]\n  Measures scan plus cold/warm native thumbnail import.\nbite bench-workflow <workflow.bite> --overwrite [--iterations 2] [run flags]\n  Measures repeated native workflow throughput and process/cache counts.\nbite bench-cancel <workflow.bite> [--after-ms 250] [run flags]\n  Measures cooperative cancellation latency during a native workflow.\nbite inspect|validate <workflow.bite>\nbite validate-node <file>\nbite validate-format <file>\nbite validate-workflow <file>\nbite expr --params <json> <expression>\nbite schemas <directory>\nbite nodes|formats|version");
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
                value => return Err(format!("Unexpected bench-cancel argument: {value}")),
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
            &mut |_, _, _| {},
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
                "imagemagick_processes_started": host.processes,
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
                value => return Err(format!("Unexpected bench-workflow argument: {value}")),
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
            let processes = host.processes;
            let hits = host.metadata_cache_stats.hits;
            let misses = host.metadata_cache_stats.misses;
            host.reset_peak_child_memory();
            let started = Instant::now();
            let result = bite_core::execution::run_workflow(
                &workflow.workflow.graph,
                &registry,
                &mut host,
                &options,
                &mut |_, _, _| {},
            )?;
            measurements.push(serde_json::json!({
                "iteration": iteration + 1,
                "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
                "processed": result.processed,
                "skipped": result.skipped,
                "failed": result.failed,
                "imagemagick_processes": host.processes - processes,
                "peak_child_memory_bytes": host.peak_child_memory_bytes(),
                "metadata_cache_hits": host.metadata_cache_stats.hits - hits,
                "metadata_cache_misses": host.metadata_cache_stats.misses - misses,
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
                flag => return Err(format!("Unknown bench-import option: {flag}")),
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
            return Err(format!("No images found in: {}", root.display()));
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
        let cold_processes = host.processes;
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
                    "processes": host.processes - cold_processes,
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
        if args.len() < 2 {
            return Err("Missing workflow file argument.\nUsage: bite run <workflow.bite>".into());
        }
        let mut options = bite_core::execution::RunOptions::default();
        let planning = args[0] == "plan";
        let observing = args[0] == "observe";
        let mut facts_out = None;
        let mut facts = bite_core::plan::PlanningFacts::default();
        let cancelled = options.cancelled.clone();
        ctrlc::set_handler(move || cancelled.store(true, std::sync::atomic::Ordering::Relaxed))
            .map_err(|e| format!("Cannot install cancellation handler: {e}"))?;
        let mut i = 2;
        while i < args.len() {
            if observing && args[i] == "--facts-out" {
                i += 1;
                facts_out = Some(
                    args.get(i)
                        .ok_or("--facts-out requires a JSON file")?
                        .into(),
                );
            } else if planning && args[i] == "--facts" {
                i += 1;
                let path = args.get(i).ok_or("--facts requires a JSON file")?;
                facts = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("Invalid planning facts: {e}"))?;
            } else if args[i] == "--overwrite" {
                options.overwrite = true;
            } else if args[i] == "--jobs" {
                i += 1;
                options.jobs = args
                    .get(i)
                    .ok_or("--jobs requires a value")?
                    .parse()
                    .map_err(|_| "invalid --jobs")?;
                if options.jobs == 0 || options.jobs > 256 {
                    return Err("--jobs must be between 1 and 256".into());
                }
            } else if let Some(flag) = args[i].strip_prefix("--") {
                if args.get(i + 1).is_some_and(|s| !s.starts_with("--")) {
                    i += 1;
                    let path = std::path::PathBuf::from(&args[i]);
                    options.named_paths.insert(
                        flag.into(),
                        if path.is_absolute() {
                            path
                        } else {
                            env::current_dir().map_err(|e| e.to_string())?.join(path)
                        },
                    );
                }
            }
            i += 1;
        }
        let registry = load_registry()?;
        let text = fs::read_to_string(&args[1])
            .map_err(|_| format!("Cannot read workflow: {}", args[1]))?;
        let loaded = bite_core::workflow::load(&text, &registry)?;
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
        if planning {
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
            return Ok(());
        }
        let result = bite_core::execution::run_workflow(
            &loaded.workflow.graph,
            &registry,
            &mut host,
            &options,
            &mut |done, total, file| println!("[{done}/{total}] {}", file.display()),
        )?;
        println!(
            "Done: {} processed, {} skipped, {} failed",
            result.processed, result.skipped, result.failed
        );
        for error in &result.errors {
            eprintln!("[Bite] Failed to process {error}");
        }
        return Ok(());
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
    if args[0] == "version" {
        println!("bite {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if ["inspect", "validate"].contains(&args[0].as_str()) {
        if args.len() != 2 {
            return Err("usage: bite inspect|validate workflow.bite".into());
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
            return Err("usage: bite expr --params '{\"width\":1024}' 'width / 2'".into());
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
        return Err(format!(
            "Unknown command: {:?}. Run \"bite --help\" for usage.",
            args[0]
        ));
    }
    if args.len() != 2 {
        return Err("expected a command and one path; use --help".into());
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
        cmd => return Err(format!("unknown command {cmd:?}")),
    };
    result.map_err(|errors| errors.join("\n"))?;
    println!("OK: {label}");
    Ok(())
}
fn load_registry() -> Result<bite_core::Registry, String> {
    let mut roots = Vec::new();
    if let Some(root) = env::var_os("BITE_RESOURCES") {
        roots.push(std::path::PathBuf::from(root));
    }
    if let Ok(exe) = env::current_exe() {
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
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[Bite] {e}");
            ExitCode::FAILURE
        }
    }
}
