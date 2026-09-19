use bite_expr::*;
use serde_json::json;
fn eval(source: &str, context: serde_json::Value) -> Result<Value, Error> {
    let ctx: Context = serde_json::from_value(context).unwrap();
    let types = ctx.iter().map(|(k, v)| (k.clone(), v.kind())).collect();
    Expression::compile(source, &types)?.evaluate(&ctx)
}
#[test]
fn arithmetic_vectors_and_legacy_semantics() {
    for (expr, ctx, expected) in [
        ("1 + 2 * 3", json!({}), "7"),
        ("-7 / 0", json!({}), "0"),
        ("a + b", json!({"a":[1,2,3],"b":[4]}), "5,2,3"),
        ("a * b", json!({"a":[1,2,3],"b":[4]}), "4,2,3"),
        ("a / b", json!({"a":[1,2,3],"b":[4]}), "0.25,0,0"),
        ("2 - a", json!({"a":[3,4]}), "-1,-2"),
        ("lerp(a,b,0.5)", json!({"a":[1,2,3],"b":[5]}), "3,2,3"),
        ("round(-1.5)", json!({}), "-1"),
        ("normalize(v)", json!({"v":[3,0,4]}), "0.6,0,0.8"),
        ("normalize(v)", json!({"v":[0,0]}), "0,0"),
        ("v > 4.9", json!({"v":[3,0,4]}), "true"),
        ("bool(v)", json!({"v":[0,0]}), "false"),
        ("v.z", json!({"v":[1,2]}), "0"),
        ("dot(a,b)", json!({"a":2,"b":[3,4]}), "6"),
    ] {
        assert_eq!(eval(expr, ctx).unwrap().text(), expected, "{expr}");
    }
}
#[test]
fn lazy_evaluation_and_metadata() {
    let types = [
        ("image.width".into(), Type::Number),
        ("image.exif.Make".into(), Type::String),
    ]
    .into();
    let e = Expression::compile("if(true, image.width, float(image.exif.Make))", &types).unwrap();
    assert_eq!(e.metadata.len(), 2);
    assert_eq!(
        e.evaluate(&[("image.width".into(), Value::Int(128))].into())
            .unwrap(),
        Value::Int(128)
    );
    assert_eq!(
        eval("false && int('bad')", json!({})).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        eval("true || int('bad')", json!({})).unwrap(),
        Value::Bool(true)
    );
}
#[test]
fn strings_arguments_and_switches() {
    assert_eq!(
        eval("format('{}x{}', width, 12)", json!({"width":42}))
            .unwrap()
            .text(),
        "42x12"
    );
    assert_eq!(
        eval("format('日本語 {}', 'text')", json!({}))
            .unwrap()
            .text(),
        "日本語 text"
    );
    assert_eq!(
        eval("rgba(c)", json!({"c":[1,0.5,0,0.25]})).unwrap().text(),
        "rgba(255,128,0,0.25)"
    );
    let specs: Vec<bite_schema::ArgSpec> = serde_json::from_value(json!(["-label",{"expr":"label"},{"when":"false","args":["omitted"]},{"switch":"mode","cases":{"x":["x-value"]},"default":["fallback"]}])).unwrap();
    let compiled = CompiledArg::compile_all(
        &specs,
        &[
            ("label".into(), Type::String),
            ("mode".into(), Type::String),
        ]
        .into(),
    )
    .unwrap();
    for label in ["", "a b", "$(command); -delete 0"] {
        assert_eq!(
            CompiledArg::resolve_all(
                &compiled,
                &[
                    ("label".into(), Value::String(label.into())),
                    ("mode".into(), Value::String("unknown".into()))
                ]
                .into()
            )
            .unwrap(),
            vec!["-label", label, "fallback"]
        );
    }
}
#[test]
fn diagnostics_and_resource_limits() {
    let types = [("width".into(), Type::Number), ("v".into(), Type::Vector)].into();
    let error = Expression::compile("widht + 1", &types).unwrap_err();
    assert!(error.message.contains("width"));
    assert_eq!((error.start, error.end), (0, 5));
    for expression in [
        "let x = 1",
        "x = 1",
        "for(1)",
        "unknown(1)",
        "abs(1,2)",
        "'text' - 1",
        "v.q",
        "width.x",
        "(1",
        "1;2",
        "'unterminated",
        "",
        "1e9999",
    ] {
        assert!(
            Expression::compile(expression, &types).is_err(),
            "{expression}"
        );
    }
    assert!(Expression::compile(&"(".repeat(100), &types).is_err());
    assert!(Expression::compile(&"1+".repeat(3000), &types).is_err());
    assert!(Expression::compile(&format!("{}1", "1+".repeat(100)), &types).is_err());
    assert!(eval("format('{} {}',1)", json!({})).is_err());
    assert!(eval("clamp(1,2,0)", json!({})).is_err());
}
