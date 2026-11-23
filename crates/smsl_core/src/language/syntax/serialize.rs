mod branches;
mod expressions;

pub(crate) use expressions::expression;

use crate::language::Machine;
use crate::language::model::{Document, EventDeclaration, EventField, Value};
use crate::language::modules::Module;
use std::fmt::Write;

pub fn to_smsl(machine: &Machine) -> String {
    to_smsl_document(machine.document())
}

pub fn to_smsl_document(document: &Document) -> String {
    let mut source = header(&document.version);
    write_events(&mut source, &document.events);
    for branch in &document.branches {
        branches::write_branch(&mut source, branch);
    }
    source
}

pub fn to_smsl_module(module: &Module) -> String {
    let mut source = header(&module.version);
    for import in &module.imports {
        let names = import
            .names
            .iter()
            .map(|name| {
                if name.name == name.alias {
                    identifier(&name.name)
                } else {
                    format!("{} as {}", identifier(&name.name), identifier(&name.alias))
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            source,
            "import {{ {names} }} from {};",
            quoted(&import.source)
        )
        .unwrap();
    }
    if !module.exports.is_empty() {
        writeln!(
            source,
            "export {{ {} }};",
            module
                .exports
                .iter()
                .map(|name| identifier(name))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    for declaration in &module.domains {
        writeln!(
            source,
            "domain {} = {};",
            identifier(&declaration.name),
            domain(&declaration.values, declaration.range)
        )
        .unwrap();
    }
    write_events(&mut source, &module.events);
    for template in &module.templates {
        branches::write_parameterized_branch(
            &mut source,
            &template.branch,
            Some(&template.parameters),
        );
    }
    for branch in &module.branches {
        branches::write_branch(&mut source, branch);
    }
    for instance in &module.instances {
        let arguments = instance
            .arguments
            .iter()
            .map(|(name, value)| format!("{} = {}", identifier(name), expressions::datum(value)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            source,
            "\ninstance {}: {}({arguments});",
            identifier(&instance.name),
            identifier(&instance.template)
        )
        .unwrap();
    }
    source
}

fn header(version: &str) -> String {
    format!("smsl {};\n", quoted(version))
}

fn write_events(source: &mut String, events: &[EventDeclaration]) {
    for event in events {
        writeln!(
            source,
            "event {}({});",
            identifier(&event.name),
            parameters(&event.fields)
        )
        .unwrap();
    }
}

fn parameters(fields: &[EventField]) -> String {
    fields
        .iter()
        .map(|field| {
            format!(
                "{}: {}{}",
                identifier(&field.name),
                expressions::data_type(field.data_type),
                if field.nullable { "?" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn value(value: &Value) -> String {
    match value {
        Value::Bool(value) => value.to_string(),
        Value::String(value) => quoted(value),
        Value::UInt(value) => value.to_string(),
    }
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("Strings are JSON serializable")
}

pub(crate) fn identifier(value: &str) -> String {
    let mut chars = value.chars();
    let bare = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if bare && !crate::source::lexer::is_keyword(value) {
        value.to_owned()
    } else {
        quoted(value)
    }
}

fn domain(values: &[Value], range: Option<crate::model::UIntRange>) -> String {
    match range {
        Some(range) => format!("uint[{}..{}]", range.min, range.max),
        None => format!(
            "[{}]",
            values.iter().map(value).collect::<Vec<_>>().join(", ")
        ),
    }
}
