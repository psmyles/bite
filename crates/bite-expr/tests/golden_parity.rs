use bite_expr::{definition::*, *};
use bite_schema::{FormatDefinition, NodeDefinition, ParamValue};
use serde_json::{json, Value as J};
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn metadata(case: &J) -> Context {
    let mut ctx = Context::new();
    for (key, t) in metadata_types() {
        let legacy = key.strip_prefix("image.").unwrap();
        let old = match legacy {
            "size" => "sizeBytes",
            "bit_depth" => "bitDepth",
            "dpi_x" => "dpiX",
            "dpi_y" => "dpiY",
            other => other,
        };
        let v = if let Some(exif) = old.strip_prefix("exif.") {
            case["meta"]["exif"][exif].clone()
        } else {
            case["meta"][old].clone()
        };
        ctx.insert(
            key,
            if v.is_null() {
                if t == Type::String {
                    Value::String(String::new())
                } else {
                    Value::Int(0)
                }
            } else {
                serde_json::from_value(v).unwrap()
            },
        );
    }
    ctx
}
fn assert_value(actual: &Value, expected: &J, label: &str) {
    if let Some(marker) = expected.get("$number").and_then(J::as_str) {
        assert_eq!(actual.text(), marker, "{label}");
    } else if let Some(n) = expected.as_f64() {
        let got = actual.scalar().unwrap();
        assert!(
            (got - n).abs() <= 1e-12 * n.abs().max(1.0),
            "{label}: {got} != {n}"
        );
    } else if let Some(items) = expected.as_array() {
        let Value::Vector(v) = actual else {
            panic!("{label}: expected vector")
        };
        assert_eq!(v.len(), items.len(), "{label}");
        for (v, e) in v.iter().zip(items) {
            assert_value(&Value::Float(*v), e, label);
        }
    } else {
        assert_eq!(serde_json::to_value(actual).unwrap(), *expected, "{label}");
    }
}
#[test]
fn all_converted_definitions_compile_and_match_valid_reference_cases() {
    let mut count = 0;
    for file in fs::read_dir(root().join("node-definitions-v2")).unwrap() {
        let file = file.unwrap();
        let d: NodeDefinition =
            serde_json::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
        let compiled = CompiledDefinition::compile(d).unwrap();
        let reference = root().join(format!(
            "tests/golden/nodes/{}.json",
            compiled.definition.id
        ));
        if !reference.exists() {
            assert_eq!(compiled.definition.id, "posterize");
            continue;
        }
        let golden: J = serde_json::from_str(&fs::read_to_string(reference).unwrap()).unwrap();
        for raw_case in golden["cases"].as_array().unwrap() {
            let mut defaults_case;
            let case = if raw_case["name"].as_str().unwrap().starts_with("missing") {
                defaults_case = golden["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| {
                        c["name"]
                            == if raw_case["name"].as_str().unwrap().ends_with("no-meta") {
                                "defaults-no-meta"
                            } else {
                                "defaults"
                            }
                    })
                    .unwrap()
                    .clone();
                defaults_case["params"] = json!({});
                &defaults_case
            } else {
                raw_case
            };
            let values = case["params"].as_object().unwrap();
            if values.iter().any(|(k, v)| {
                compiled
                    .definition
                    .params
                    .iter()
                    .find(|p| p.name == *k)
                    .is_some_and(|p| {
                        !p.readonly
                            && serde_json::from_value::<ParamValue>(v.clone())
                                .map_or(true, |v| !p.accepts(&v))
                    })
            }) {
                let supplied: bite_schema::Params =
                    serde_json::from_value(case["params"].clone()).unwrap();
                assert!(resolve_context(&compiled.definition.params, &supplied).is_err());
                continue;
            }
            let mut ctx: Context = values
                .iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<Value>(v.clone())
                        .ok()
                        .map(|v| (k.clone(), v))
                })
                .collect();
            if case["params"] == json!({}) {
                ctx = resolve_context(&compiled.definition.params, &bite_schema::Params::new())
                    .unwrap();
            }
            ctx.extend(metadata(case));
            let label = format!("{} / {}", compiled.definition.id, case["name"]);
            match &compiled.implementation {
                CompiledImplementation::Imagemagick(args) => {
                    let actual = CompiledArg::resolve_all(args, &ctx)
                        .unwrap_or_else(|e| panic!("{label}: {e}"));
                    assert_eq!(json!(actual), case["expected_args"], "{label}");
                    count += 1;
                }
                CompiledImplementation::Compute(outputs) => {
                    for (key, e) in outputs {
                        if let Some(expected) = case["expected_params"].get(key) {
                            let actual = e
                                .evaluate(&ctx)
                                .unwrap_or_else(|e| panic!("{label}/{key}: {e}"));
                            assert_value(&actual, expected, &format!("{label}/{key}"));
                            count += 1;
                        }
                    }
                }
                CompiledImplementation::Native(_) => {}
            }
        }
    }
    assert!(count >= 486, "only {count} comparisons");
    println!("{count} node argument/result comparisons");
}
#[test]
fn formats_match_reference_with_documented_png_override() {
    for file in fs::read_dir(root().join("format-definitions-v2")).unwrap() {
        let d: FormatDefinition =
            serde_json::from_str(&fs::read_to_string(file.unwrap().path()).unwrap()).unwrap();
        d.validate().unwrap();
        let args = CompiledArg::compile_all(&d.args, &parameter_types(&d.params)).unwrap();
        let golden: J = serde_json::from_str(
            &fs::read_to_string(
                root().join(format!("tests/golden/formats/{}.json", d.id.to_lowercase())),
            )
            .unwrap(),
        )
        .unwrap();
        for case in golden["cases"].as_array().unwrap() {
            if case["params"].as_object().unwrap().iter().any(|(k, v)| {
                d.params.iter().find(|p| p.name == *k).is_some_and(|p| {
                    serde_json::from_value::<ParamValue>(v.clone()).map_or(true, |v| !p.accepts(&v))
                })
            }) {
                let supplied: bite_schema::Params =
                    serde_json::from_value(case["params"].clone()).unwrap();
                assert!(resolve_context(&d.params, &supplied).is_err());
                continue;
            }
            let mut ctx = default_context(&d.params);
            let supplied: Context = serde_json::from_value(case["params"].clone()).unwrap();
            ctx.extend(supplied);
            if let Some(quality) = ctx.get("quality").cloned() {
                let key = format!("{}_quality", d.id.to_lowercase());
                if !case["params"].as_object().unwrap().contains_key(&key) {
                    ctx.insert(key, quality);
                }
            }
            let actual = CompiledArg::resolve_all(&args, &ctx).unwrap();
            let mut expected: Vec<String> =
                serde_json::from_value(case["expected_args"].clone()).unwrap();
            if d.id == "PNG" {
                let i = expected.iter().position(|a| a == "-depth").unwrap();
                expected[i] = "-define".into();
                expected[i + 1] = format!("png:bit-depth={}", expected[i + 1]);
                expected.extend(["-define".into(), "png:color-type=6".into()]);
            }
            assert_eq!(actual, expected, "{} / {}", d.id, case["name"]);
        }
    }
}
