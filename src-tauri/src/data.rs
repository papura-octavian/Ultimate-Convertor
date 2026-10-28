//! JSON / YAML / TOML / CSV through one shared JSON value, and Markdown to HTML.

use serde_json::{Map, Value};
use std::path::Path;

fn read(path: &Path, ext: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    match ext {
        "json" => serde_json::from_str(text).map_err(|e| format!("JSON: {e}")),
        "yaml" | "yml" => serde_yaml::from_str(text).map_err(|e| format!("YAML: {e}")),
        "toml" => toml::from_str(text).map_err(|e| format!("TOML: {e}")),
        "csv" => read_csv(text, b','),
        "tsv" => read_csv(text, b'\t'),
        other => Err(format!("Unsupported data format: {other}")),
    }
}

fn read_csv(text: &str, delimiter: u8) -> Result<Value, String> {
    // Excel in many locales writes `;` separated CSV.
    let delimiter = if delimiter == b',' {
        let first = text.lines().next().unwrap_or("");
        if first.matches(';').count() > first.matches(',').count() { b';' } else { b',' }
    } else {
        delimiter
    };
    let mut rdr = csv::ReaderBuilder::new().delimiter(delimiter).flexible(true).from_reader(text.as_bytes());
    let headers: Vec<String> = rdr.headers().map_err(|e| format!("CSV: {e}"))?.iter().map(str::to_string).collect();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV: {e}"))?;
        let mut obj = Map::new();
        for (i, field) in rec.iter().enumerate() {
            let key = headers.get(i).cloned().unwrap_or_else(|| format!("column{}", i + 1));
            obj.insert(key, guess_scalar(field));
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

/// CSV cells are text; turn obvious numbers and booleans back into them.
fn guess_scalar(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::String(String::new());
    }
    match t {
        "true" | "TRUE" | "True" => return Value::Bool(true),
        "false" | "FALSE" | "False" => return Value::Bool(false),
        _ => {}
    }
    // Keep leading zeros (phone numbers, codes) as text.
    let leading_zero = t.len() > 1 && t.starts_with('0') && !t.starts_with("0.");
    if !leading_zero {
        if let Ok(i) = t.parse::<i64>() {
            return Value::from(i);
        }
        if let Ok(f) = t.parse::<f64>() {
            if f.is_finite() {
                return Value::from(f);
            }
        }
    }
    Value::String(s.to_string())
}

fn write(value: &Value, target: &str) -> Result<String, String> {
    match target {
        "json" => serde_json::to_string_pretty(value).map_err(|e| e.to_string()),
        "yaml" => serde_yaml::to_string(value).map_err(|e| e.to_string()),
        "toml" => {
            let table = match value {
                Value::Object(_) => strip_nulls(value.clone()),
                other => Value::Object(Map::from_iter([("items".to_string(), strip_nulls(other.clone()))])),
            };
            toml::to_string_pretty(&table).map_err(|e| format!("TOML: {e}"))
        }
        "csv" => write_csv(value),
        other => Err(format!("Unsupported data target: {other}")),
    }
}

/// TOML has no null; drop those keys rather than failing.
fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        Value::Array(a) => Value::Array(a.into_iter().filter(|v| !v.is_null()).map(strip_nulls).collect()),
        other => other,
    }
}

fn write_csv(value: &Value) -> Result<String, String> {
    // Rows: an array, the only array inside an object, or the object itself.
    let rows: Vec<Value> = match value {
        Value::Array(a) => a.clone(),
        Value::Object(m) => {
            let arrays: Vec<&Value> = m.values().filter(|v| v.is_array()).collect();
            if arrays.len() == 1 && m.len() == 1 {
                arrays[0].as_array().cloned().unwrap_or_default()
            } else {
                vec![value.clone()]
            }
        }
        other => vec![other.clone()],
    };
    let mut headers: Vec<String> = Vec::new();
    for row in &rows {
        match row {
            Value::Object(m) => {
                for k in m.keys() {
                    if !headers.contains(k) {
                        headers.push(k.clone());
                    }
                }
            }
            _ => {
                if !headers.iter().any(|h| h == "value") {
                    headers.push("value".into());
                }
            }
        }
    }
    let cell = |v: Option<&Value>| match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v @ (Value::Array(_) | Value::Object(_))) => v.to_string(),
        Some(v) => v.to_string(),
    };
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(&headers).map_err(|e| e.to_string())?;
    for row in &rows {
        let record: Vec<String> = headers
            .iter()
            .map(|h| match row {
                Value::Object(m) => cell(m.get(h)),
                other if h == "value" => cell(Some(other)),
                _ => String::new(),
            })
            .collect();
        w.write_record(&record).map_err(|e| e.to_string())?;
    }
    let bytes = w.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

pub fn convert_data(input: &Path, ext: &str, target: &str, output: &Path) -> Result<(), String> {
    let value = read(input, ext)?;
    let mut text = write(&value, target)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    // Excel only reads UTF-8 CSV correctly with a BOM.
    if target == "csv" {
        text.insert(0, '\u{feff}');
    }
    std::fs::write(output, text).map_err(|e| e.to_string())
}

pub fn markdown_to_html(input: &Path, output: &Path) -> Result<(), String> {
    use pulldown_cmark::{html, Options, Parser};
    let md = std::fs::read_to_string(input).map_err(|e| e.to_string())?;
    let parser = Parser::new_ext(
        &md,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_FOOTNOTES,
    );
    let mut body = String::new();
    html::push_html(&mut body, parser);
    let title = input.file_stem().and_then(|s| s.to_str()).unwrap_or("Document");
    let title = title.replace('&', "&amp;").replace('<', "&lt;");
    let page = format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
body {{ max-width: 760px; margin: 40px auto; padding: 0 20px; font: 16px/1.65 -apple-system, "Segoe UI", Roboto, sans-serif; color: #1d1d27; }}
h1, h2, h3 {{ line-height: 1.25; }}
code {{ background: #f2f2f7; padding: 2px 5px; border-radius: 4px; font-size: .92em; }}
pre {{ background: #f2f2f7; padding: 14px; border-radius: 8px; overflow: auto; }}
pre code {{ background: none; padding: 0; }}
table {{ border-collapse: collapse; }}
th, td {{ border: 1px solid #ddd; padding: 6px 10px; }}
blockquote {{ margin: 0; padding-left: 16px; border-left: 4px solid #ddd; color: #555; }}
img {{ max-width: 100%; }}
</style>
</head>
<body>
{body}</body>
</html>
"#
    );
    std::fs::write(output, page).map_err(|e| e.to_string())
}
