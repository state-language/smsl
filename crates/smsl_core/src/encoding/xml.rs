use std::{collections::BTreeMap, fmt::Write};

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde_json::{Map, Value};

use super::{MAX_VALUE_DEPTH, MAX_VALUE_NODES, quoted, resource, syntax, unsigned};
use crate::Diagnostic;

struct Node {
    name: String,
    attributes: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}

pub(super) fn decode(source: &str) -> Result<Value, Diagnostic> {
    if source.chars().any(|ch| !xml_character(ch)) {
        return Err(syntax("Invalid XML character"));
    }
    let mut reader = Reader::from_str(source);
    reader.config_mut().expand_empty_elements = true;
    reader.config_mut().check_comments = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0;
    let mut first = true;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| syntax(error.to_string()))?;
        let initial = first;
        first = false;
        match event {
            Event::Start(element) => {
                count += 1;
                if stack.len() >= 2 * MAX_VALUE_DEPTH + 2 || count > 3 * MAX_VALUE_NODES + 1 {
                    return Err(resource("XML wrapper depth or node limit exceeded"));
                }
                let mut attributes = BTreeMap::new();
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|error| syntax(error.to_string()))?;
                    let key = std::str::from_utf8(attribute.key.as_ref())
                        .map_err(|error| syntax(error.to_string()))?
                        .to_owned();
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Explicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|error| syntax(error.to_string()))?
                        .into_owned();
                    if attributes.insert(key, value).is_some() {
                        return Err(syntax("Duplicate XML attribute"));
                    }
                }
                stack.push(Node {
                    name: std::str::from_utf8(element.name().as_ref())
                        .map_err(|error| syntax(error.to_string()))?
                        .into(),
                    attributes,
                    text: String::new(),
                    children: Vec::new(),
                });
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| syntax("Unexpected XML end element"))?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.replace(node).is_some() {
                    return Err(syntax("Expected exactly one XML root"));
                }
            }
            Event::Text(text) => {
                let text = text.decode().map_err(|error| syntax(error.to_string()))?;
                append_text(&mut stack, &text)?;
            }
            Event::GeneralRef(reference) => {
                let name: &[u8] = reference.as_ref();
                let character = match name {
                    b"amp" => '&',
                    b"lt" => '<',
                    b"gt" => '>',
                    b"quot" => '"',
                    b"apos" => '\'',
                    _ => reference
                        .resolve_char_ref()
                        .map_err(|error| syntax(error.to_string()))?
                        .ok_or_else(|| syntax("Custom XML entities are unsupported"))?,
                };
                if !xml_character(character) {
                    return Err(syntax("Invalid XML character reference"));
                }
                append_text(&mut stack, &character.to_string())?;
            }
            Event::Comment(_) => {}
            Event::Decl(declaration) => {
                if !initial {
                    return Err(syntax("Misplaced XML declaration"));
                }
                let declaration_text = std::str::from_utf8(declaration.as_ref())
                    .map_err(|error| syntax(error.to_string()))?;
                let attributes = BytesStart::from_content(declaration_text, 3);
                for attribute in attributes.attributes() {
                    let attribute = attribute.map_err(|error| syntax(error.to_string()))?;
                    if !matches!(attribute.key.as_ref(), b"version" | b"encoding") {
                        return Err(syntax("Unexpected XML declaration attribute"));
                    }
                }
                if declaration
                    .version()
                    .map_err(|error| syntax(error.to_string()))?
                    .as_ref()
                    != b"1.0"
                {
                    return Err(syntax("SMSL XML requires version 1.0"));
                }
                if let Some(encoding) = declaration.encoding() {
                    if !encoding
                        .map_err(|error| syntax(error.to_string()))?
                        .eq_ignore_ascii_case(b"utf-8")
                    {
                        return Err(syntax("SMSL XML requires UTF-8"));
                    }
                }
            }
            Event::Eof => break,
            _ => {
                return Err(syntax(
                    "XML DTDs, processing instructions, and CDATA are unsupported",
                ));
            }
        }
    }
    if !stack.is_empty() {
        return Err(syntax("Unclosed XML element"));
    }
    let root = root.ok_or_else(|| syntax("Missing XML root"))?;
    if root.name != "smsl-value"
        || root.attributes != BTreeMap::from([("version".into(), "1".into())])
        || root.children.len() != 1
        || !trimmed(&root.text).is_empty()
    {
        return Err(syntax("Expected smsl-value version=1 with one typed value"));
    }
    value(&root.children[0], 1)
}

fn trimmed(text: &str) -> &str {
    text.trim_matches([' ', '\t', '\r', '\n'])
}

fn xml_character(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn append_text(stack: &mut [Node], text: &str) -> Result<(), Diagnostic> {
    if let Some(node) = stack.last_mut() {
        node.text.push_str(text);
    } else if !trimmed(text).is_empty() {
        return Err(syntax("Text outside XML root"));
    }
    Ok(())
}

fn value(node: &Node, depth: usize) -> Result<Value, Diagnostic> {
    if depth > MAX_VALUE_DEPTH {
        return Err(resource("XML value depth limit exceeded"));
    }
    if !node.attributes.is_empty() {
        return Err(syntax("Unexpected XML value attributes"));
    }
    match node.name.as_str() {
        "object" | "array" if trimmed(&node.text).is_empty() => {
            if node.name == "array" {
                return node
                    .children
                    .iter()
                    .map(|node| value(node, depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array);
            }
            let mut fields = Map::new();
            for member in &node.children {
                if member.name != "member"
                    || !member.attributes.is_empty()
                    || !trimmed(&member.text).is_empty()
                    || member.children.len() != 2
                {
                    return Err(syntax(
                        "XML object requires member elements with key and value",
                    ));
                }
                let key = &member.children[0];
                if key.name != "key" || !key.attributes.is_empty() || !key.children.is_empty() {
                    return Err(syntax("XML member must begin with a key"));
                }
                let key: String = serde_json::from_str(trimmed(&key.text))
                    .map_err(|error| syntax(error.to_string()))?;
                if fields.contains_key(&key) {
                    return Err(syntax(format!("Duplicate XML key {key:?}")));
                }
                fields.insert(key, value(&member.children[1], depth + 1)?);
            }
            Ok(Value::Object(fields))
        }
        "null" if node.children.is_empty() && trimmed(&node.text).is_empty() => Ok(Value::Null),
        "boolean" if node.children.is_empty() => match trimmed(&node.text) {
            "true" => Ok(true.into()),
            "false" => Ok(false.into()),
            _ => Err(syntax("Invalid XML Boolean")),
        },
        "unsigned" if node.children.is_empty() => unsigned(trimmed(&node.text)),
        "string" if node.children.is_empty() => serde_json::from_str::<String>(trimmed(&node.text))
            .map(Value::String)
            .map_err(|error| syntax(error.to_string())),
        _ => Err(syntax("Invalid XML typed value")),
    }
}

pub(super) fn encode(value: &Value) -> String {
    let mut output = String::from("<smsl-value version=\"1\">\n");
    write_value(&mut output, value, 1);
    output.push_str("</smsl-value>\n");
    output
}

fn escaped(value: &str) -> String {
    quick_xml::escape::escape(value).into_owned()
}

fn write_value(output: &mut String, value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        Value::Null => writeln!(output, "{indent}<null/>").unwrap(),
        Value::Bool(value) => writeln!(output, "{indent}<boolean>{value}</boolean>").unwrap(),
        Value::Number(value) => writeln!(output, "{indent}<unsigned>{value}</unsigned>").unwrap(),
        Value::String(value) => writeln!(
            output,
            "{indent}<string>{}</string>",
            escaped(&quoted(value))
        )
        .unwrap(),
        Value::Array(values) => {
            writeln!(output, "{indent}<array>").unwrap();
            for value in values {
                write_value(output, value, depth + 1);
            }
            writeln!(output, "{indent}</array>").unwrap();
        }
        Value::Object(values) => {
            writeln!(output, "{indent}<object>").unwrap();
            for (key, value) in values {
                writeln!(
                    output,
                    "{indent}  <member>\n{indent}    <key>{}</key>",
                    escaped(&quoted(key))
                )
                .unwrap();
                write_value(output, value, depth + 2);
                writeln!(output, "{indent}  </member>").unwrap();
            }
            writeln!(output, "{indent}</object>").unwrap();
        }
    }
}
