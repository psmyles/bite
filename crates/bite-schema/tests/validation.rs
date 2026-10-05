use bite_schema::*;
use serde_json::{json, Value};

fn node() -> Value {
    serde_json::from_str(include_str!("../../../node-definitions/posterize.json")).unwrap()
}
fn workflow() -> Value {
    json!({"schema_version":2,"created_with":"test","graph":{"nodes":[{"id":"input","type":"inputNode","position":{"x":0,"y":0},"data":{"label":"Input","definitionId":"","params":{"thumbnailSize":256,"cliName":"in"}}}],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}})
}
#[test]
fn validates_reference_and_schema_versions() {
    let n: NodeDefinition = serde_json::from_value(node()).unwrap();
    n.validate().unwrap();
    let mut n = n;
    n.schema_version = 1;
    assert!(n
        .validate()
        .unwrap_err()
        .join(" ")
        .contains("schema_version"));
    let w: Workflow = serde_json::from_value(workflow()).unwrap();
    w.validate().unwrap();
}
#[test]
fn rejects_ambiguous_arguments_and_executable_fields() {
    for value in [
        json!({"expr":"x","args":[]}),
        json!({"when":"true","args":[],"switch":"x"}),
        json!({"expr":"x","unknown":true}),
    ] {
        assert!(serde_json::from_value::<ArgSpec>(value).is_err());
    }
    let mut n = node();
    n["command_js"] = json!("return []");
    assert!(serde_json::from_value::<NodeDefinition>(n).is_err());
    let mut w = workflow();
    w["graph"]["nodes"][0]["data"]["params"]["payload"] = json!({"expr":"x"});
    assert!(serde_json::from_value::<Workflow>(w).is_err());
    let mut w = workflow();
    w["graph"]["nodes"][0]["data"]["params"]["__compute_js__"] = json!("bad");
    assert!(serde_json::from_value::<Workflow>(w)
        .unwrap()
        .validate()
        .is_err());
}
#[test]
fn contextual_diagnostics() {
    let mut n = node();
    n["params"][0].as_object_mut().unwrap().remove("max");
    let n: NodeDefinition = serde_json::from_value(n).unwrap();
    assert!(n
        .validate()
        .unwrap_err()
        .join(" ")
        .contains("param \"levels\": slider requires"));
    let mut n = node();
    n["outputs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"output","type":"image","label":"Duplicate"}));
    assert!(serde_json::from_value::<NodeDefinition>(n)
        .unwrap()
        .validate()
        .unwrap_err()
        .join(" ")
        .contains("duplicate"));
}
#[test]
fn validates_builtin_params_and_edges() {
    let mut w = workflow();
    w["graph"]["nodes"][0]["data"]["params"]["thumbnailSize"] = json!("wrong");
    assert!(serde_json::from_value::<Workflow>(w)
        .unwrap()
        .validate()
        .is_err());
    let mut w = workflow();
    w["graph"]["edges"] = json!([{"id":"e","source":"input","sourceHandle":"out-0","target":"missing","targetHandle":"in:input"}]);
    let errs = serde_json::from_value::<Workflow>(w)
        .unwrap()
        .validate()
        .unwrap_err()
        .join(" ");
    assert!(errs.contains("unknown endpoint") && errs.contains("invalid handle"));
}
#[test]
fn structured_values_round_trip() {
    for value in [
        json!({"type":"rename_blocks","blocks":[{"type":"text","value":"prefix"}]}),
        json!({"type":"set_suffixes","suffixes":["_normal","_diffuse"]}),
        json!({"type":"text_slots","slots":["0","1"]}),
    ] {
        let p: ParamValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(p).unwrap(), value);
    }
}
#[test]
fn checked_in_schemas_match_types() {
    for (name, schema) in json_schemas() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas")
            .join(name);
        let checked: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        assert_eq!(checked, schema);
    }
}
