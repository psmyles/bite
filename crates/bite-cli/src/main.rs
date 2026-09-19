use bite_schema::{FormatDefinition, NodeDefinition, Workflow};
use std::{env, fs, process::ExitCode};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("bite run <workflow.bite> [--<cliName> <path>] [--overwrite]\n  --overwrite  Overwrite existing output files (default: skip)\nbite plan <workflow.bite> [--<cliName> <path>] [--overwrite] [--facts <json>]\n  Reports complete=false when image analysis facts are required.\nbite inspect|validate <workflow.bite>\nbite validate-node <file>\nbite validate-format <file>\nbite validate-workflow <file>\nbite expr --params <json> <expression>\nbite schemas <directory>\nbite nodes|formats|version");
        return Ok(());
    }
    if args[0] == "run" || args[0] == "plan" {
        if args.len() < 2 {
            return Err("Missing workflow file argument.\nUsage: bite run <workflow.bite>".into());
        }
        let mut options = bite_core::execution::RunOptions::default();
        let planning = args[0] == "plan";
        let mut facts = bite_core::plan::PlanningFacts::default();
        let cancelled = options.cancelled.clone();
        ctrlc::set_handler(move || cancelled.store(true, std::sync::atomic::Ordering::Relaxed))
            .map_err(|e| format!("Cannot install cancellation handler: {e}"))?;
        let mut i = 2;
        while i < args.len() {
            if planning && args[i] == "--facts" {
                i += 1;
                let path = args.get(i).ok_or("--facts requires a JSON file")?;
                facts = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("Invalid planning facts: {e}"))?;
            } else if args[i] == "--overwrite" {
                options.overwrite = true;
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
        if planning {
            use bite_core::execution::ImageHost;
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
        for suffix in ["-v2", ""] {
            let nodes = root.join(format!("node-definitions{suffix}"));
            let formats = root.join(format!("format-definitions{suffix}"));
            if nodes.is_dir() && formats.is_dir() {
                return bite_core::Registry::load(&nodes, &formats);
            }
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
