use crate::{
    Diagnostic,
    application::{ApplicationDocument, ApplicationInput, ApplicationSource},
    architecture::{Port, SystemDefinition},
};
use serde_json::Value;
use std::fmt::Write;

pub fn format_value(value: &Value) -> Result<String, Diagnostic> {
    crate::encoding::validate_value(value)?;
    let text = value_text(value);
    crate::source::lexer::validate_source(&text)?;
    Ok(text)
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(value_text).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(values) => {
            if values.is_empty() {
                return "{}".into();
            }
            format!(
                "{{ {} }}",
                values
                    .iter()
                    .map(|(key, value)| format!("{} = {};", name(key), value_text(value)))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
        _ => value.to_string(),
    }
}

fn name(text: &str) -> String {
    if !text.is_empty()
        && !matches!(text, "true" | "false" | "null")
        && text.split('.').all(|part| {
            let mut chars = part.chars();
            !matches!(part, "true" | "false" | "null")
                && chars
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
                && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
    {
        text.into()
    } else {
        quote(text)
    }
}
fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("string")
}

struct Writer {
    text: String,
    depth: usize,
}
impl Writer {
    fn new(kind: &str, id: &str) -> Self {
        Self {
            text: format!("smsl \"0.2\";\n\n{kind} {} {{\n", name(id)),
            depth: 1,
        }
    }
    fn line(&mut self, text: impl AsRef<str>) {
        writeln!(self.text, "{}{}", "    ".repeat(self.depth), text.as_ref())
            .expect("string writer");
    }
    fn open(&mut self, text: impl AsRef<str>) {
        self.line(format!("{} {{", text.as_ref()));
        self.depth += 1;
    }
    fn close(&mut self) {
        self.depth -= 1;
        self.line("}");
    }
    fn finish(mut self) -> Result<String, Diagnostic> {
        self.close();
        crate::source::lexer::validate_source(&self.text)?;
        Ok(self.text)
    }
}

pub(crate) fn system(definition: &SystemDefinition) -> Result<String, Diagnostic> {
    crate::encoding::validate_value(&serde_json::to_value(definition).expect("system document"))?;
    let mut out = Writer::new("system", &definition.id);
    if definition.system_version != 1 {
        out.line(format!("version {};", definition.system_version));
    }
    out.line(format!("profile {};", name(&definition.profile)));
    for (alias, value) in &definition.aliases {
        out.line(format!("alias {} = {};", name(alias), quote(value)));
    }
    for import in &definition.imports {
        out.line(format!("import {};", quote(import)));
    }
    for contract in &definition.contracts {
        out.open(format!("contract {}", name(&contract.id)));
        for (id, port) in &contract.inputs {
            out.line(port_text("input", id, port));
        }
        for (id, port) in &contract.outputs {
            out.line(port_text("output", id, port));
        }
        out.line(format!(
            "effect {};",
            serde_json::to_value(contract.effect)
                .expect("effect")
                .as_str()
                .expect("effect name")
        ));
        out.close();
    }
    for component in &definition.components {
        out.open(format!(
            "component {}: {}",
            name(&component.id),
            name(&component.role)
        ));
        if let Some(label) = &component.label {
            out.line(format!("label {};", quote(label)));
        }
        for block in &component.blocks {
            out.open(format!(
                "block {}: {}",
                name(&block.id),
                name(&block.contract)
            ));
            if let Some(provider) = &block.provider {
                out.line(format!("provider {};", quote(provider)));
            }
            out.line(format!("config {};", value_text(&block.config)));
            out.close();
        }
        out.close();
    }
    for connection in &definition.connections {
        let delay = if connection.delay == 0 {
            String::new()
        } else {
            format!(" delay {}", connection.delay)
        };
        out.line(format!(
            "connect {} -> {}{delay};",
            name(&connection.from),
            name(&connection.to)
        ));
    }
    out.finish()
}
fn port_text(direction: &str, id: &str, port: &Port) -> String {
    let mut text = format!("{direction} {}: {}", name(id), name(&port.schema));
    if !port.required {
        text.push_str(" optional");
    }
    if let Some(unit) = &port.unit {
        write!(text, " unit {}", quote(unit)).expect("string");
    }
    if let Some(frame) = &port.frame {
        write!(text, " frame {}", quote(frame)).expect("string");
    }
    if port.freshness == crate::architecture::Freshness::Any {
        text.push_str(" freshness any");
    }
    text.push(';');
    text
}

pub(crate) fn application(document: &ApplicationDocument) -> Result<String, Diagnostic> {
    crate::encoding::validate_value(
        &serde_json::to_value(document).expect("application document"),
    )?;
    let mut out = Writer::new("application", &document.id);
    if document.application_version != 1 {
        out.line(format!("version {};", document.application_version));
    }
    for (id, source) in &document.models {
        out.line(format!("model {} {};", name(id), source_text(source)));
    }
    out.line(format!("system {};", source_text(&document.system)));
    for instance in &document.instances {
        out.open(format!(
            "instance {}: {}",
            name(&instance.id),
            name(&instance.model)
        ));
        out.line(format!("scope {};", quote(&instance.context.scope)));
        out.line(format!("subject {};", quote(&instance.context.subject)));
        out.line(format!("clock {};", quote(&instance.context.clock)));
        out.close();
    }
    for (address, binding) in &document.bindings {
        out.open(format!(
            "bind {} to {}",
            name(address),
            name(&binding.instance)
        ));
        for (id, input) in &binding.inputs {
            let text = match input {
                ApplicationInput::Value { value } => format!("= {}", value_text(value)),
                ApplicationInput::Host { host: true } => "from host".into(),
                ApplicationInput::Host { host: false } => {
                    return Err(Diagnostic::new(
                        "invalid_application",
                        "Host input must be true",
                        None,
                    ));
                }
                ApplicationInput::Profile { profile, config } => {
                    let mut text = format!("from profile {}", name(profile));
                    if let Some(config) = config {
                        write!(
                            text,
                            " config {}",
                            value_text(&Value::Object(config.clone()))
                        )
                        .expect("string");
                    }
                    text
                }
            };
            out.line(format!("input {} {text};", name(id)));
        }
        if let Some(outputs) = &binding.outputs {
            out.open("outputs");
            for (port, profile) in outputs {
                out.line(format!("output {} as {};", name(port), name(profile)));
            }
            out.close();
        }
        if let Some(config) = &binding.configuration {
            out.open("configuration");
            for (key, reference) in config {
                out.line(format!(
                    "{} from model {};",
                    name(key),
                    name(&reference.model)
                ));
            }
            out.close();
        }
        if let Some(lifecycle) = &binding.lifecycle {
            out.line(format!(
                "lifecycle to {} event {};",
                name(&lifecycle.instance),
                name(&lifecycle.event)
            ));
        }
        out.close();
    }
    out.finish()
}
fn source_text(source: &ApplicationSource) -> String {
    match source {
        ApplicationSource::Reference { r#ref } => format!("from {}", quote(r#ref)),
        ApplicationSource::Embedded { text, format } => format!(
            "embedded {} {}",
            serde_json::to_value(format)
                .expect("format")
                .as_str()
                .expect("format name"),
            quote(text)
        ),
    }
}

pub(crate) fn execution(definition: &crate::execution::Definition) -> Result<String, Diagnostic> {
    let value = serde_json::to_value(definition).expect("execution definition");
    crate::encoding::validate_value(&value)?;
    let mut out = Writer::new("execution", &definition.id);
    if definition.version != 1 {
        out.line(format!("version {};", definition.version));
    }
    out.line(format!("initial {};", name(&definition.root)));
    for (id, block) in value["blocks"].as_object().expect("blocks object") {
        out.open(format!(
            "block {}: {}",
            name(id),
            name(block["mechanism"].as_str().expect("mechanism"))
        ));
        for (key, value) in block.as_object().expect("block object") {
            if key == "expression" {
                let expression: crate::model::Expr =
                    serde_json::from_value(value.clone()).expect("typed expression");
                out.line(format!(
                    "when {};",
                    crate::language::syntax::serialize::expression(&expression)
                ));
            } else if key != "mechanism" {
                out.line(format!("input {} = {};", name(key), value_text(value)));
            }
        }
        out.close();
    }
    out.finish()
}
