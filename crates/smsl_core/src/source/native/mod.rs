//! Native system, application and execution declarations with JSON-pointer source ranges.
mod application;
mod execution;
mod parser;
mod serialize;
mod system;
mod value;

use crate::Diagnostic;
use serde_json::Value;
use std::{collections::BTreeMap, ops::Range};

pub use serialize::format_value;
pub(crate) use serialize::{
    application as encode_application, execution as encode_execution, system as encode_system,
};

#[derive(Clone, Debug)]
pub struct ParsedDocument {
    pub kind: &'static str,
    pub value: Value,
    pub spans: BTreeMap<String, Range<usize>>,
}

pub fn parse(source: &str) -> Result<ParsedDocument, Diagnostic> {
    parser::Parser::new(source)?.document()
}

/// Recognize the declaration after a native version header, including comments.
/// The body need not be syntactically complete.
pub fn kind(source: &str) -> Option<&'static str> {
    fn skip(mut text: &str) -> &str {
        loop {
            text = text.trim_start();
            if let Some(comment) = text.strip_prefix("//") {
                text = comment.find(['\n', '\r']).map_or("", |end| &comment[end..]);
            } else {
                return text;
            }
        }
    }
    let source = skip(source).strip_prefix("smsl")?;
    let source = skip(source).strip_prefix('"')?;
    let end = source.find('"')?;
    let source = skip(&source[end + 1..]).strip_prefix(';')?;
    let source = skip(source);
    ["system", "application", "execution"]
        .into_iter()
        .find(|&kind| {
            source.strip_prefix(kind).is_some_and(|tail| {
                tail.is_empty()
                    || !tail.starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
            })
        })
}
