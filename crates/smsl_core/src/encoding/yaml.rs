use std::fmt::Write;

use saphyr_parser::{Event, Parser, ScalarStyle};
use serde_json::{Map, Value};

use super::{MAX_VALUE_DEPTH, MAX_VALUE_NODES, quoted, resource, syntax, unsigned};
use crate::Diagnostic;

enum Frame {
    Array(Vec<Value>),
    Object(Map<String, Value>, Option<String>),
}

pub(super) fn decode(source: &str) -> Result<Value, Diagnostic> {
    if source.lines().any(|line| line.starts_with('%')) {
        return Err(syntax("YAML directives are outside the SMSL profile"));
    }
    let mut frames = Vec::new();
    let mut root = None;
    let mut documents = 0;
    let mut nodes = 0;
    for event in Parser::new_from_str(source) {
        let (event, _) = event.map_err(|error| syntax(error.to_string()))?;
        let sequence = matches!(&event, Event::SequenceStart(..));
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    return Err(syntax("Expected exactly one YAML document"));
                }
            }
            Event::Alias(_) => return Err(syntax("YAML aliases are not supported")),
            Event::Scalar(text, style, anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(syntax("YAML anchors and tags are not supported"));
                }
                if !matches!(frames.last(), Some(Frame::Object(_, None))) {
                    nodes += 1;
                }
                if nodes > MAX_VALUE_NODES {
                    return Err(resource("YAML node limit exceeded"));
                }
                let value = scalar(&text, style)?;
                append(&mut frames, &mut root, value)?;
            }
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(syntax("YAML anchors and tags are not supported"));
                }
                nodes += 1;
                if frames.len() >= MAX_VALUE_DEPTH || nodes > MAX_VALUE_NODES {
                    return Err(resource("YAML depth or node limit exceeded"));
                }
                frames.push(if sequence {
                    Frame::Array(Vec::new())
                } else {
                    Frame::Object(Map::new(), None)
                });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let value = match frames
                    .pop()
                    .ok_or_else(|| syntax("Unexpected YAML container end"))?
                {
                    Frame::Array(values) => Value::Array(values),
                    Frame::Object(values, None) => Value::Object(values),
                    Frame::Object(_, Some(_)) => return Err(syntax("Missing YAML mapping value")),
                };
                append(&mut frames, &mut root, value)?;
            }
            Event::Nothing => return Err(syntax("Unexpected YAML event")),
        }
    }
    root.ok_or_else(|| syntax("Expected one YAML value"))
}

fn append(frames: &mut [Frame], root: &mut Option<Value>, value: Value) -> Result<(), Diagnostic> {
    match frames.last_mut() {
        Some(Frame::Array(values)) => values.push(value),
        Some(Frame::Object(values, key)) => {
            if let Some(key) = key.take() {
                values.insert(key, value);
            } else {
                let Value::String(name) = value else {
                    return Err(syntax("YAML mapping keys must be strings"));
                };
                if values.contains_key(&name) {
                    return Err(syntax(format!("Duplicate YAML key {name:?}")));
                }
                *key = Some(name);
            }
        }
        None => {
            if root.replace(value).is_some() {
                return Err(syntax("Expected one YAML value"));
            }
        }
    }
    Ok(())
}

fn scalar(text: &str, style: ScalarStyle) -> Result<Value, Diagnostic> {
    if style != ScalarStyle::Plain {
        return Ok(text.into());
    }
    match text {
        "null" | "~" | "" => Ok(Value::Null),
        "true" => Ok(true.into()),
        "false" => Ok(false.into()),
        _ if text.starts_with(|ch: char| ch.is_ascii_digit()) => unsigned(text),
        _ if text.starts_with(['+', '-'])
            && text[1..].starts_with(|ch: char| ch.is_ascii_digit() || ch == '.') =>
        {
            Err(syntax(
                "YAML signed and floating-point numbers are unsupported",
            ))
        }
        _ if text.starts_with('.') => Err(syntax(
            "YAML floating-point scalars are unsupported; quote strings",
        )),
        _ => Ok(text.into()),
    }
}

pub(super) fn encode(value: &Value) -> String {
    let mut output = String::new();
    write_value(&mut output, value, 0);
    output
}

fn inline(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".into()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(quoted(value)),
        Value::Array(values) if values.is_empty() => Some("[]".into()),
        Value::Object(values) if values.is_empty() => Some("{}".into()),
        _ => None,
    }
}

fn write_value(output: &mut String, value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    if let Some(value) = inline(value) {
        writeln!(output, "{indent}{value}").unwrap();
        return;
    }
    match value {
        Value::Array(values) => {
            for value in values {
                if let Some(value) = inline(value) {
                    writeln!(output, "{indent}- {value}").unwrap();
                } else {
                    writeln!(output, "{indent}-").unwrap();
                    write_value(output, value, depth + 1);
                }
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                let key = quoted(key);
                if let Some(value) = inline(value) {
                    writeln!(output, "{indent}{key}: {value}").unwrap();
                } else {
                    writeln!(output, "{indent}{key}:").unwrap();
                    write_value(output, value, depth + 1);
                }
            }
        }
        _ => unreachable!("scalar handled above"),
    }
}
