use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::diagnostic::Diagnostic;
use crate::language::model::{Branch, Document, Expr};
use crate::language::validation::memory::expressions::resolve_branch;

type Field = (String, String);

pub(crate) fn order(document: &Document) -> Result<Vec<Field>, Diagnostic> {
    let branches: HashMap<_, _> = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), branch))
        .collect();
    let mut nodes = BTreeMap::new();
    for branch in &document.branches {
        for fact in &branch.facts {
            if let Some(expr) = &fact.derived {
                nodes.insert((branch.name.clone(), fact.name.clone()), (branch, expr));
            }
        }
        for label in branch.labels.iter().chain(&branch.named_states) {
            nodes.insert(
                (branch.name.clone(), label.name.clone()),
                (branch, &label.expression),
            );
        }
        for binding in &branch.sub_branches {
            let child = branches[binding.branch.as_str()];
            nodes.insert(
                (branch.name.clone(), binding.fact.clone()),
                (child, child.activating.as_ref().unwrap()),
            );
        }
    }
    let mut dependencies: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    let mut dependents: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for (key, (context, expr)) in &nodes {
        let required = dependencies.entry(key.clone()).or_default();
        for (source, field) in reads(expr) {
            if let Some(target) = resolve_branch(source, context, &branches) {
                let dependency = (target.name.clone(), field.to_owned());
                if nodes.contains_key(&dependency) {
                    required.insert(dependency.clone());
                    dependents
                        .entry(dependency)
                        .or_default()
                        .insert(key.clone());
                }
            }
        }
    }
    let mut ready: BTreeSet<_> = dependencies
        .iter()
        .filter(|(_, deps)| deps.is_empty())
        .map(|(key, _)| key.clone())
        .collect();
    let mut result = Vec::with_capacity(nodes.len());
    while let Some(key) = ready.pop_first() {
        if let Some(targets) = dependents.get(&key) {
            for target in targets {
                let remaining = dependencies.get_mut(target).unwrap();
                remaining.remove(&key);
                if remaining.is_empty() {
                    ready.insert(target.clone());
                }
            }
        }
        result.push(key);
    }
    if result.len() != nodes.len() {
        let fields = dependencies
            .iter()
            .filter(|(_, deps)| !deps.is_empty())
            .map(|((branch, field), _)| format!("{branch}.{field}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Diagnostic::new(
            "derived_cycle",
            format!("Cyclic derived fields: {fields}"),
            None,
        ));
    }
    Ok(result)
}

pub(crate) fn reads(expr: &Expr) -> Vec<(&str, &str)> {
    let mut pending = vec![expr];
    let mut result = Vec::new();
    while let Some(expr) = pending.pop() {
        match expr {
            Expr::Read { source, field } => result.push((source.as_str(), field.as_str())),
            Expr::Not { expr } | Expr::IsUnknown { expr } => pending.push(expr),
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
            | Expr::Rem { left, right } => pending.extend([left.as_ref(), right.as_ref()]),
            Expr::Literal { .. } => {}
        }
    }
    result
}

pub(crate) fn expressions(branch: &Branch) -> Vec<&Expr> {
    let mut result = Vec::new();
    result.extend(branch.activating.iter());
    result.extend(branch.facts.iter().filter_map(|fact| fact.derived.as_ref()));
    result.extend(
        branch
            .labels
            .iter()
            .chain(&branch.named_states)
            .chain(&branch.invariants)
            .map(|label| &label.expression),
    );
    for operation in &branch.operations {
        result.extend(operation.guard.iter());
        result.extend(operation.postcondition.iter());
        result.extend(operation.updates.iter().map(|update| &update.value));
    }
    result.extend(
        branch
            .actions
            .iter()
            .filter_map(|action| action.precondition.as_ref()),
    );
    for reaction in branch
        .reactions
        .iter()
        .chain(branch.states.iter().flat_map(|state| &state.reactions))
    {
        result.extend(reaction.guard.iter());
        result.extend(reaction.updates.iter().map(|update| &update.value));
    }
    result.extend(
        branch
            .states
            .iter()
            .flat_map(|state| &state.transitions)
            .flat_map(|transition| transition.updates.iter().map(|update| &update.value)),
    );
    result
}
