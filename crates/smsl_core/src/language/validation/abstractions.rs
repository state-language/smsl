use std::collections::{HashMap, HashSet};

use crate::{
    Diagnostic,
    language::validation::memory::expressions::resolve_branch,
    model::{Branch, Datum, Document, Expr},
};

pub(super) fn validate(document: &Document) -> Result<(), Diagnostic> {
    let branches = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), branch))
        .collect();
    for (branch_index, branch) in document.branches.iter().enumerate() {
        for (index, state) in branch.named_states.iter().enumerate() {
            pure(branch, &state.name, &branches).map_err(|error| {
                error.at_path(format!(
                    "/branches/{branch_index}/named_states/{index}/expression"
                ))
            })?;
        }
        let mut names = HashSet::new();
        for (index, abstraction) in branch.abstractions.iter().enumerate() {
            let path = format!("/branches/{branch_index}/abstractions/{index}");
            if abstraction.name.is_empty() || !names.insert(&abstraction.name) {
                return Err(error("Abstraction names must be nonempty and distinct")
                    .at_path(format!("{path}/name")));
            }
            if abstraction.facts.is_empty() {
                return Err(error("An abstraction must select at least one fact")
                    .at_path(format!("{path}/facts")));
            }
            let mut facts = HashSet::new();
            for (index, name) in abstraction.facts.iter().enumerate() {
                let path = format!("{path}/facts/{index}");
                if !facts.insert(name) {
                    return Err(error("Abstraction facts must be distinct").at_path(path));
                }
                let Some(fact) = branch.facts.iter().find(|fact| fact.name == *name) else {
                    return Err(error(format!("Unknown abstraction fact {name:?}")).at_path(path));
                };
                let bound = branch
                    .sub_branches
                    .iter()
                    .any(|binding| binding.fact == *name);
                if fact.derived.is_none() && !bound {
                    return Err(
                        error("Abstractions select derived facts or child summaries").at_path(path),
                    );
                }
                pure(branch, name, &branches).map_err(|error| error.at_path(path))?;
            }
        }
    }
    Ok(())
}

fn pure<'a>(
    branch: &'a Branch,
    field: &'a str,
    branches: &HashMap<&str, &'a Branch>,
) -> Result<(), Diagnostic> {
    let mut fields = vec![(branch, field)];
    let mut expressions = Vec::new();
    let mut visited = HashSet::new();
    while !fields.is_empty() || !expressions.is_empty() {
        while let Some((branch, field)) = fields.pop() {
            if !visited.insert((branch.name.as_str(), field)) {
                continue;
            }
            if field == "state" && branch.initial.is_some() {
                continue;
            }
            if let Some(binding) = branch
                .sub_branches
                .iter()
                .find(|binding| binding.fact == field)
            {
                let child = branches[binding.branch.as_str()];
                expressions.push((child, child.activating.as_ref().unwrap()));
            } else if let Some(fact) = branch.facts.iter().find(|fact| fact.name == field) {
                if let Some(expression) = &fact.derived {
                    expressions.push((branch, expression));
                }
            } else if let Some(label) = branch
                .labels
                .iter()
                .chain(&branch.named_states)
                .find(|label| label.name == field)
            {
                expressions.push((branch, &label.expression));
            } else {
                return Err(error(format!(
                    "Abstraction input {}.{field} must be a finite fact, label or named control",
                    branch.name
                )));
            }
        }
        let Some((branch, expression)) = expressions.pop() else {
            continue;
        };
        match expression {
            Expr::Literal {
                value: Datum::Bool(_) | Datum::String(_) | Datum::Time(_),
            } => {}
            Expr::Read { source, field } => {
                let target = resolve_branch(source, branch, branches)
                    .ok_or_else(|| error("Abstractions cannot read event or action context"))?;
                fields.push((target, field));
            }
            Expr::Not { expr } => expressions.push((branch, expr)),
            Expr::And { left, right }
            | Expr::Or { left, right }
            | Expr::Eq { left, right }
            | Expr::Ne { left, right }
            | Expr::Lt { left, right }
            | Expr::Le { left, right }
            | Expr::Gt { left, right }
            | Expr::Ge { left, right }
            | Expr::Add { left, right }
            | Expr::Sub { left, right }
            | Expr::Mul { left, right }
            | Expr::Div { left, right }
            | Expr::Rem { left, right } => {
                expressions.extend([(branch, left.as_ref()), (branch, right.as_ref())]);
            }
            Expr::Literal { .. } | Expr::IsUnknown { .. } => {
                return Err(error(
                    "Abstractions require finite value expressions, independent of missing knowledge",
                ));
            }
        }
    }
    Ok(())
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_abstraction", message, None)
}
