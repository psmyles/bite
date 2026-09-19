use bite_core::{
    execution::{self, ImageHost, RunOptions},
    workflow, Registry,
};
use bite_expr::Context;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

struct FailingHost {
    cancelled: Arc<AtomicBool>,
    cancel_on_failure: bool,
    calls: usize,
}
impl ImageHost for FailingHost {
    fn metadata(&mut self, _: &Path, _: bool) -> Result<Context, String> {
        Ok(Context::new())
    }
    fn capture(&mut self, _: &[String]) -> Result<String, String> {
        Err("unexpected capture".into())
    }
    fn run(&mut self, args: &[String]) -> Result<(), String> {
        self.calls += 1;
        fs::write(args.last().unwrap(), b"partial or successful image").unwrap();
        if self.calls == 1 {
            if self.cancel_on_failure {
                self.cancelled.store(true, Ordering::Relaxed);
            }
            Err("injected image failure".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_images_preserve_existing_outputs_and_cancellation_still_stops_batch() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    let graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-01-fastpath.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    for cancel in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        fs::create_dir(&input).unwrap();
        fs::create_dir(&output).unwrap();
        for name in ["a.png", "b.png"] {
            fs::write(input.join(name), b"input").unwrap();
        }
        fs::write(output.join("a.png"), b"original").unwrap();
        let options = RunOptions {
            named_paths: [("in".into(), input), ("out".into(), output.clone())].into(),
            overwrite: true,
            ..RunOptions::default()
        };
        let mut host = FailingHost {
            cancelled: options.cancelled.clone(),
            cancel_on_failure: cancel,
            calls: 0,
        };
        let result =
            execution::run_workflow(&graph, &registry, &mut host, &options, &mut |_, _, _| {});
        assert_eq!(fs::read(output.join("a.png")).unwrap(), b"original");
        if cancel {
            assert!(result.is_err());
            assert_eq!(host.calls, 1);
            assert!(!output.join("b.png").exists());
        } else {
            let result = result.unwrap();
            assert_eq!((result.processed, result.skipped, result.failed), (1, 0, 1));
            assert_eq!(result.errors.len(), 1);
            assert!(result.errors[0].contains("a.png"));
            assert_eq!(result.outputs, vec![output.join("b.png")]);
            assert_eq!(host.calls, 2);
        }
        assert_eq!(
            fs::read_dir(&output).unwrap().count(),
            if cancel { 1 } else { 2 },
            "temporary output leaked"
        );
    }
}
