//! One-shot, reviewable migration. Templates are tokenized mechanically; the
//! seven JavaScript nodes and seven formats below are explicitly translated.
use serde_json::{json, Value};
use std::{fs, path::Path};
fn e(s: &str) -> Value {
    json!({"expr":s})
}
fn when(s: &str, args: Value) -> Value {
    json!({"when":s,"args":args})
}
fn compute(pairs: &[(&str, &str)]) -> Value {
    json!({"type":"compute","outputs":pairs.iter().map(|(k,v)|(k.to_string(),json!(v))).collect::<serde_json::Map<_,_>>()})
}
fn im(args: Value) -> Value {
    json!({"type":"imagemagick","args":args})
}
fn template(s: &str) -> Value {
    let mut format = String::new();
    let mut params = Vec::new();
    let mut remaining = s;
    while let Some(start) = remaining.find("{{") {
        format.push_str(&remaining[..start]);
        format.push_str("{}");
        let end = remaining[start + 2..].find("}}").unwrap() + start + 2;
        params.push(remaining[start + 2..end].to_owned());
        remaining = &remaining[end + 2..];
    }
    format.push_str(remaining);
    if params.is_empty() {
        json!(s)
    } else if params.len() == 1 && format == "{}" {
        e(&params[0])
    } else {
        e(&format!(
            "format({}, {})",
            serde_json::to_string(&format).unwrap(),
            params.join(", ")
        ))
    }
}
fn implementation(d: &Value) -> Value {
    let id = d["id"].as_str().unwrap();
    if let Some(t) = d["command_template"].as_str() {
        return im(Value::Array(t.split_whitespace().map(template).collect()));
    }
    match id {
        "extend-canvas" => im(json!([when("left > 0",json!(["-gravity","West","-background","none","-splice",e("format('{}x0', left)")])),when("right > 0",json!(["-gravity","East","-background","none","-splice",e("format('{}x0', right)")])),when("top > 0",json!(["-gravity","North","-background","none","-splice",e("format('0x{}', top)")])),when("bottom > 0",json!(["-gravity","South","-background","none","-splice",e("format('0x{}', bottom)")])),when("left > 0 || right > 0 || top > 0 || bottom > 0",json!(["-gravity","NorthWest"]))])),
        "flip" => im(json!([e("if(axis == 'vertical', '-flip', '-flop')")])),
        "hue_offset" => im(json!(["-modulate",e("format('100,100,{}', 100 + hue / 1.8)")])),
        "outline" => im(json!(["(","+clone","-alpha","extract","-morphology","Dilate",e("format('Disk:{}', size)"),"-background",e("str(color)"),"-alpha","Shape",")","+swap","-composite"])),
        "pixelate" => im(json!(["-filter","Point","-resize",e("format('{}%', 100 / max(2, round(block_size)))"),"-filter","Point","-resize",e("format('{}%', max(2, round(block_size)) * 100)")])),
        "premultiply-alpha" => im(json!(["-alpha",e("if(mode == 'premultiply', 'Associate', 'Disassociate')")])),
        "resize-nn" => im(json!(["-filter","Point","-resize",e("format('{}x{}{}', width, height, if(ignore_aspect, '!', ''))")])),
        "resize" => im(json!([when("round(density) > 0",json!(["-density",e("round(density)"),"-units","PixelsPerInch"])),"-filter",e("filter"),"-resize",{"switch":"mode","cases":{"relative":[e("if(preserve_aspect, format('{}%', max(1, scale)), format('{}%x{}%!', max(1, scale_width), max(1, scale_height)))")]},"default":[e("if(preserve_aspect, if(anchor == 'height', format('x{}', max(1, round(height))), str(max(1, round(width)))), format('{}x{}!', max(1, round(width)), max(1, round(height))))")]}])),
        "math_add" => compute(&[("result","a + b")]), "math_subtract" => compute(&[("result","a - b")]), "math_multiply" => compute(&[("result","a * b")]), "math_divide" => compute(&[("result","a / b")]), "math_power" => compute(&[("result","pow(base, exponent)")]), "math_lerp" => compute(&[("result","lerp(a, b, t)")]),
        "logic_and" => compute(&[("result","bool(a) && bool(b)")]), "logic_or" => compute(&[("result","bool(a) || bool(b)")]), "logic_not" => compute(&[("result","!bool(a)")]), "logic_branch" => compute(&[("result","if(condition, value_true, value_false)")]),
        "logic_comparison" => compute(&[("result","if(operator == 'equal', float(a) == float(b), if(operator == 'not equal', float(a) != float(b), if(operator == 'greater than', float(a) > float(b), if(operator == 'less than', float(a) < float(b), if(operator == 'greater or equal', float(a) >= float(b), if(operator == 'less or equal', float(a) <= float(b), false))))))")]),
        "vec_math_dot" => compute(&[("result","dot(a,b)")]), "vec_math_length" => compute(&[("result","length(vec)")]), "vec_math_normalize" => compute(&[("result","normalize(vec)")]),
        "split_vec" => compute(&[("x","vec.x"),("y","vec.y"),("z","vec.z"),("w","vec.w")]),
        "append_vec" => compute(&[("result","if(int(dimensions) == 2, vec2(x,y), if(int(dimensions) == 3, vec3(x,y,z), vec4(x,y,z,w)))")]),
        "value_color" => compute(&[("rgba","vec4(color.x,color.y,color.z,color.w)"),("rgb","vec3(color.x,color.y,color.z)"),("r","color.x"),("g","color.y"),("b","color.z"),("a","color.w")]),
        "value_vector2" => compute(&[("xy","vec2(vec.x,vec.y)"),("x","vec.x"),("y","vec.y")]),
        "value_vector3" => compute(&[("xyz","vec3(vec.x,vec.y,vec.z)"),("x","vec.x"),("y","vec.y"),("z","vec.z")]),
        "value_vector4" => compute(&[("xyzw","vec4(vec.x,vec.y,vec.z,vec.w)"),("x","vec.x"),("y","vec.y"),("z","vec.z"),("w","vec.w")]),
        "value_boolean" | "value_float" | "value_string" | "folderpath" => compute(&[]),
        "text_filter" => compute(&[("result","starts_with(if(match_case, input, lower(input)), if(match_case, prefix, lower(prefix))) && ends_with(if(match_case, input, lower(input)), if(match_case, suffix, lower(suffix))) && contains(if(match_case, input, lower(input)), if(match_case, contains, lower(contains)))")]),
        "prop_name" => compute(&[("value","if(strip_extension, strip_extension(image.name), image.name)")]),
        "prop_path" => compute(&[("value","if(strip_filename, dirname(image.path), image.path)")]),
        "prop_filetype" => compute(&[("value","image.extension")]), "prop_bitdepth" => compute(&[("value","image.bit_depth")]),
        "prop_dimensions" => compute(&[("width","image.width"),("height","image.height")]),
        "prop_resolution" => compute(&[("dpi_x","image.dpi_x"),("dpi_y","image.dpi_y")]),
        "prop_size" => compute(&[("value","image.size / if(unit == 'KB', 1024, if(unit == 'MB', 1048576, if(unit == 'GB', 1073741824, 1)))")]),
        "prop_power_of_two" => compute(&[("width_ok","power_of_two(image.width)"),("height_ok","power_of_two(image.height)"),("result","power_of_two(image.width) && power_of_two(image.height)")]),
        "prop_exif" => compute(&[("camera_make","image.exif.Make"),("camera_model","image.exif.Model"),("lens","if(bool(image.exif.LensModel), image.exif.LensModel, image.exif.LensMake)"),("exposure_time","image.exif.ExposureTime"),("shutter_speed","image.exif.ShutterSpeedValue"),("aperture","if(bool(rational(image.exif.FNumber)), rational(image.exif.FNumber), rational(image.exif.ApertureValue))"),("iso","if(bool(parse_int(image.exif.PhotographicSensitivity)), parse_int(image.exif.PhotographicSensitivity), parse_int(image.exif.ISOSpeedRatings))"),("focal_length","rational(image.exif.FocalLength)"),("date_taken","image.exif.DateTimeOriginal")]),
        _ => { let executor = d["executor"].as_str().unwrap_or_else(|| panic!("unclassified node {id}")); json!({"type":"native","executor":executor}) },
    }
}
fn common(d: &mut Value) {
    d["schema_version"] = json!(2);
    if d.get("version").is_none() {
        d["version"] = json!("1.0.0");
    }
    let rules = d.as_object_mut().unwrap().remove("params_visibility");
    for p in d["params"].as_array_mut().unwrap() {
        if p["type"] == "any" {
            p["type"] = json!("value");
        }
        if p.get("default").is_some_and(Value::is_null) {
            p.as_object_mut().unwrap().remove("default");
        }
        if let Some(rules) = rules.as_ref().and_then(Value::as_array) {
            for rule in rules {
                if p["name"] == rule["show"] {
                    p["visible_when"] = json!(format!(
                        "{} == {}",
                        rule["when"]["param"].as_str().unwrap(),
                        rule["when"]["eq"]
                    ));
                }
            }
        }
    }
}
fn main() {
    fs::create_dir_all("node-definitions-v2").unwrap();
    fs::create_dir_all("format-definitions-v2").unwrap();
    for entry in fs::read_dir("node-definitions").unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let mut d: Value =
            serde_json::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap();
        let implementation = implementation(&d);
        let id = d["id"].as_str().unwrap().to_owned();
        common(&mut d);
        for dir in ["inputs", "outputs"] {
            for (i, p) in d[dir].as_array_mut().unwrap().iter_mut().enumerate() {
                p["name"] = json!(if id == "channel_split" && dir == "outputs"
                    || id == "channel_merge" && dir == "inputs"
                {
                    ["r", "g", "b", "a"][i].to_owned()
                } else if i == 0 {
                    if dir == "inputs" { "input" } else { "output" }.into()
                } else {
                    format!("{}{}", if dir == "inputs" { "input" } else { "output" }, i)
                });
            }
        }
        for key in [
            "executor",
            "command_template",
            "command_js",
            "compute_js",
            "needs_image_meta",
        ] {
            d.as_object_mut().unwrap().remove(key);
        }
        d["implementation"] = implementation;
        if id == "process_as_set" {
            for p in d["params"].as_array_mut().unwrap() {
                if p["name"] == "suffixes" {
                    p["type"] = json!("set_suffixes");
                    p["default"] = json!({"type":"set_suffixes","suffixes":[]});
                }
            }
        }
        if id == "rename" {
            d["params"].as_array_mut().unwrap().push(json!({"name":"blocks","label":"Blocks","type":"rename_blocks","default":{"type":"rename_blocks","blocks":[]},"noPort":true}));
        }
        if id == "resize" {
            for p in d["params"].as_array_mut().unwrap() {
                let visibility = match p["name"].as_str().unwrap() {
                    "width" => {
                        Some("mode == 'absolute' && (!preserve_aspect || anchor == 'width')")
                    }
                    "height" => {
                        Some("mode == 'absolute' && (!preserve_aspect || anchor == 'height')")
                    }
                    "scale" => Some("mode == 'relative' && preserve_aspect"),
                    "scale_width" | "scale_height" => {
                        Some("mode == 'relative' && !preserve_aspect")
                    }
                    "anchor" => Some("mode == 'absolute' && preserve_aspect"),
                    _ => None,
                };
                if let Some(v) = visibility {
                    p["visible_when"] = json!(v);
                }
            }
        }
        let typed: bite_schema::NodeDefinition =
            serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
        bite_expr::definition::CompiledDefinition::compile(typed)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        fs::write(
            Path::new("node-definitions-v2").join(entry.file_name()),
            serde_json::to_string_pretty(&d).unwrap() + "\n",
        )
        .unwrap();
    }
    for entry in fs::read_dir("format-definitions").unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let mut d: Value =
            serde_json::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap();
        common(&mut d);
        d["args"] = match d["id"].as_str().unwrap() {
            "AVIF" => json!([
                "-quality",
                e("avif_quality"),
                "-define",
                e("format('avif:speed={}',avif_speed)")
            ]),
            "BMP" => json!([]),
            "JPEG" => json!([
                "-quality",
                e("jpeg_quality"),
                "-sampling-factor",
                e("jpeg_sampling"),
                when("jpeg_progressive", json!(["-interlace", "Plane"]))
            ]),
            "PNG" => json!([
                "-define",
                e("format('png:compression-level={}',png_compression)"),
                "-define",
                e("format('png:bit-depth={}',png_depth)"),
                "-define",
                "png:color-type=6"
            ]),
            "TGA" => json!(["-compress", e("if(tga_rle,'RLE','None')")]),
            "TIFF" => json!(["-compress", e("tiff_compression")]),
            "WEBP" => json!([
                when("webp_lossless", json!(["-define", "webp:lossless=true"])),
                when("!webp_lossless", json!(["-quality", e("webp_quality")])),
                "-define",
                e("format('webp:method={}',webp_method)")
            ]),
            id => panic!("unclassified format {id}"),
        };
        d.as_object_mut().unwrap().remove("args_js");
        let typed: bite_schema::FormatDefinition = serde_json::from_value(d.clone()).unwrap();
        typed.validate().unwrap();
        bite_expr::CompiledArg::compile_all(
            &typed.args,
            &bite_expr::definition::parameter_types(&typed.params),
        )
        .unwrap();
        fs::write(
            Path::new("format-definitions-v2").join(entry.file_name()),
            serde_json::to_string_pretty(&d).unwrap() + "\n",
        )
        .unwrap();
    }
}
