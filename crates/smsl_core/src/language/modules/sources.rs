use std::collections::BTreeMap;

use super::{Source, SourceText, link};
use crate::{Diagnostic, Machine};

pub fn link_sources(
    root: &str,
    sources: &BTreeMap<String, SourceText>,
) -> Result<Machine, Diagnostic> {
    let root = normalize(root)?;
    let read = |id: String| {
        let value = sources
            .get(&id)
            .ok_or_else(|| error(format!("Missing source {id:?}")))?;
        Ok(Source {
            id,
            text: value.text.clone(),
            format: value.format,
        })
    };
    link(read(root)?, &mut |importer: &str, source: &str| {
        let path = if source.starts_with('/') {
            source.to_owned()
        } else if let Some((parent, _)) = importer.rsplit_once('/') {
            format!("{parent}/{source}")
        } else {
            source.to_owned()
        };
        read(normalize(&path)?)
    })
}

fn normalize(path: &str) -> Result<String, Diagnostic> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts
                    .pop()
                    .ok_or_else(|| error("Source path escapes the source map root"))?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(error("Source identity must name a file"));
    }
    Ok(format!(
        "{}{}",
        if path.starts_with('/') { "/" } else { "" },
        parts.join("/")
    ))
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_source", message, None)
}
