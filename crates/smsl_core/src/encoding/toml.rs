use std::{collections::BTreeSet, fmt::Write};

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{MAX_VALUE_DEPTH, MAX_VALUE_NODES, quoted, resource, syntax, unsigned};
use crate::Diagnostic;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    format: String,
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum Node {
    Null {},
    Boolean {
        value: bool,
    },
    Unsigned {
        value: String,
    },
    String {
        value: String,
    },
    Array {
        children: Vec<usize>,
    },
    Object {
        keys: Vec<String>,
        children: Vec<usize>,
    },
}

pub(super) fn decode(source: &str) -> Result<Value, Diagnostic> {
    preflight(source)?;
    let archive: Archive = ::toml::from_str(source).map_err(|error| syntax(error.to_string()))?;
    if archive.format != "smsl-value-1" || archive.nodes.is_empty() {
        return Err(syntax("Expected smsl-value-1 with a root node"));
    }
    if archive.nodes.len() > MAX_VALUE_NODES {
        return Err(resource("TOML node limit exceeded"));
    }
    let mut seen = BTreeSet::new();
    let value = resolve(&archive.nodes, 0, 1, &mut seen)?;
    if seen.len() != archive.nodes.len() {
        return Err(syntax("TOML contains unreachable value nodes"));
    }
    Ok(value)
}

fn preflight(source: &str) -> Result<(), Diagnostic> {
    let bytes = source.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    let mut brackets = 0usize;
    for (index, &byte) in bytes.iter().enumerate() {
        if comment {
            comment = !matches!(byte, b'\r' | b'\n');
            continue;
        }
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' && delimiter == b'"' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
            continue;
        }
        match byte {
            b'#' => comment = true,
            b'"' | b'\'' => {
                if bytes.get(index + 1) == Some(&byte) && bytes.get(index + 2) == Some(&byte) {
                    return Err(syntax(
                        "TOML profile uses single-line basic or literal strings",
                    ));
                }
                quote = Some(byte);
            }
            b'.' | b'{' | b'}' => {
                return Err(syntax(
                    "TOML profile excludes dotted keys, floats, and inline tables",
                ));
            }
            b'[' => {
                brackets += 1;
                if brackets > 2 {
                    return Err(resource(
                        "TOML profile permits only flat node tables and flat arrays",
                    ));
                }
            }
            b']' => brackets = brackets.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn resolve(
    nodes: &[Node],
    index: usize,
    depth: usize,
    seen: &mut BTreeSet<usize>,
) -> Result<Value, Diagnostic> {
    if depth > MAX_VALUE_DEPTH {
        return Err(resource("TOML value depth limit exceeded"));
    }
    if !seen.insert(index) {
        return Err(syntax("TOML child nodes cannot be reused"));
    }
    let node = nodes
        .get(index)
        .ok_or_else(|| syntax("TOML child node does not exist"))?;
    let child = |child: usize, seen: &mut BTreeSet<usize>| {
        if child <= index {
            return Err(syntax("TOML child indices must point forward"));
        }
        resolve(nodes, child, depth + 1, seen)
    };
    match node {
        Node::Null {} => Ok(Value::Null),
        Node::Boolean { value } => Ok((*value).into()),
        Node::Unsigned { value } => unsigned(value),
        Node::String { value } => Ok(value.clone().into()),
        Node::Array { children } => children
            .iter()
            .map(|index| child(*index, seen))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Node::Object { keys, children } => {
            if keys.len() != children.len() {
                return Err(syntax(
                    "TOML object keys and children must have equal lengths",
                ));
            }
            let mut fields = Map::new();
            for (key, index) in keys.iter().zip(children) {
                if fields.contains_key(key) {
                    return Err(syntax(format!("Duplicate TOML object key {key:?}")));
                }
                fields.insert(key.clone(), child(*index, seen)?);
            }
            Ok(Value::Object(fields))
        }
    }
}

pub(super) fn encode(value: &Value) -> String {
    let mut nodes = Vec::new();
    flatten(value, &mut nodes);
    let mut output = String::from("format = \"smsl-value-1\"\n");
    for node in nodes {
        output.push_str("\n[[nodes]]\n");
        match node {
            Node::Null {} => output.push_str("type = \"null\"\n"),
            Node::Boolean { value } => {
                writeln!(output, "type = \"boolean\"\nvalue = {value}").unwrap()
            }
            Node::Unsigned { value } => {
                writeln!(output, "type = \"unsigned\"\nvalue = {}", quoted(&value)).unwrap()
            }
            Node::String { value } => {
                writeln!(output, "type = \"string\"\nvalue = {}", quoted(&value)).unwrap()
            }
            Node::Array { children } => {
                writeln!(output, "type = \"array\"\nchildren = {children:?}").unwrap()
            }
            Node::Object { keys, children } => writeln!(
                output,
                "type = \"object\"\nkeys = [{}]\nchildren = {children:?}",
                keys.iter()
                    .map(|key| quoted(key))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .unwrap(),
        }
    }
    output
}

fn flatten(value: &Value, nodes: &mut Vec<Node>) -> usize {
    let index = nodes.len();
    nodes.push(Node::Null {});
    nodes[index] = match value {
        Value::Null => Node::Null {},
        Value::Bool(value) => Node::Boolean { value: *value },
        Value::Number(value) => Node::Unsigned {
            value: value.to_string(),
        },
        Value::String(value) => Node::String {
            value: value.clone(),
        },
        Value::Array(values) => Node::Array {
            children: values.iter().map(|value| flatten(value, nodes)).collect(),
        },
        Value::Object(fields) => Node::Object {
            keys: fields.keys().cloned().collect(),
            children: fields.values().map(|value| flatten(value, nodes)).collect(),
        },
    };
    index
}
