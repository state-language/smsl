use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, compile,
    model::{Document, EventDeclaration, Expr},
};

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_composition", message, None)
}

fn event_names(document: &Document) -> BTreeSet<&str> {
    let mut names: BTreeSet<_> = document
        .events
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    for branch in &document.branches {
        for reaction in branch
            .reactions
            .iter()
            .chain(branch.states.iter().flat_map(|state| &state.reactions))
        {
            names.insert(&reaction.event);
            names.extend(reaction.emit.iter().map(String::as_str));
        }
        names.extend(
            branch
                .actions
                .iter()
                .flat_map(|action| action.outcomes.iter().map(String::as_str)),
        );
    }
    names
}

pub fn map_events(
    document: &Document,
    mapping: &BTreeMap<String, String>,
) -> Result<Document, Diagnostic> {
    crate::language::limits::check_document(document)?;
    let names = event_names(document);
    let unknown: Vec<_> = mapping
        .keys()
        .filter(|name| !names.contains(name.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(error(format!(
            "Unknown event names in mapping: {unknown:?}"
        )));
    }
    if mapping.values().any(String::is_empty) {
        return Err(error("Mapped event names must be nonempty strings"));
    }
    let mut result = document.clone();
    let rename = |name: &mut String| {
        if let Some(target) = mapping.get(name) {
            name.clone_from(target);
        }
    };
    for event in &mut result.events {
        rename(&mut event.name);
    }
    for branch in &mut result.branches {
        for reaction in branch.reactions.iter_mut().chain(
            branch
                .states
                .iter_mut()
                .flat_map(|state| &mut state.reactions),
        ) {
            rename(&mut reaction.event);
            for name in &mut reaction.emit {
                rename(name);
            }
        }
        for action in &mut branch.actions {
            for outcome in &mut action.outcomes {
                rename(outcome);
            }
        }
    }
    result.events = merge_events(result.events)?;
    crate::language::limits::document(&mut result)?;
    Ok(result)
}

pub fn namespace(document: &Document, prefix: &str, events: bool) -> Result<Document, Diagnostic> {
    crate::language::limits::check_document(document)?;
    if prefix.is_empty() {
        return Err(error("Namespace prefix must be a nonempty string"));
    }
    let names: BTreeMap<_, _> = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), format!("{prefix}.{}", branch.name)))
        .collect();
    let mut result = document.clone();
    for branch in &mut result.branches {
        branch.name.clone_from(&names[branch.name.as_str()]);
        for binding in &mut branch.sub_branches {
            if let Some(name) = names.get(binding.branch.as_str()) {
                binding.branch.clone_from(name);
            }
        }
        for read in &mut branch.reads {
            if let Some(name) = names.get(read.branch.as_str()) {
                read.branch.clone_from(name);
            }
        }
        let aliases: BTreeSet<_> = branch.reads.iter().map(|read| read.alias.clone()).collect();
        super::expressions::visit(branch, &mut |expression| {
            if let Expr::Read { source, .. } = expression
                && !matches!(source.as_str(), "self" | "event" | "payload" | "args")
                && !aliases.contains(source)
                && let Some(name) = names.get(source.as_str())
            {
                source.clone_from(name);
            }
            Ok(())
        })?;
    }
    if events {
        let mapping = event_names(&result)
            .into_iter()
            .map(|name| (name.to_owned(), format!("{prefix}.{name}")))
            .collect();
        map_events(&result, &mapping)
    } else {
        crate::language::limits::document(&mut result)?;
        Ok(result)
    }
}

pub fn compose(documents: &[Document]) -> Result<Document, Diagnostic> {
    let first = documents
        .first()
        .ok_or_else(|| error("Composition requires at least one definition"))?;
    if documents
        .iter()
        .any(|document| document.version != first.version)
    {
        return Err(error(
            "Composed definitions must use the same language version",
        ));
    }
    for document in documents {
        crate::language::limits::check_document(document)?;
    }
    let mut result = Document {
        version: first.version.clone(),
        events: merge_events(
            documents
                .iter()
                .flat_map(|document| document.events.iter().cloned()),
        )?,
        branches: documents
            .iter()
            .flat_map(|document| document.branches.iter().cloned())
            .collect(),
    };
    crate::language::limits::document(&mut result)?;
    compile(result.clone())?;
    Ok(result)
}

fn signature(event: &EventDeclaration) -> Vec<(&str, u8, bool)> {
    let mut fields: Vec<_> = event
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field.data_type as u8, field.nullable))
        .collect();
    fields.sort_unstable();
    fields
}

fn merge_events(
    events: impl IntoIterator<Item = EventDeclaration>,
) -> Result<Vec<EventDeclaration>, Diagnostic> {
    let mut indices = BTreeMap::new();
    let mut merged: Vec<EventDeclaration> = Vec::new();
    for event in events {
        if let Some(&index) = indices.get(&event.name) {
            if signature(&merged[index]) != signature(&event) {
                return Err(error(format!(
                    "Conflicting event declaration: {:?}",
                    event.name
                )));
            }
        } else {
            indices.insert(event.name.clone(), merged.len());
            merged.push(event);
        }
    }
    Ok(merged)
}
