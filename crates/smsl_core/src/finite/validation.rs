use std::collections::BTreeSet;

use crate::{
    Diagnostic, Machine,
    diagnostic::key,
    model::{Branch, Datum, Expr},
    planning::{Goal, Operator},
    runtime::RuntimeState,
};

use super::planning_error;

fn reason(code: &str, message: impl Into<String>, path: &str) -> Diagnostic {
    Diagnostic::new(code, message, None).at_path(path)
}

pub(crate) fn goal(machine: &Machine, goal: &Goal) -> Result<(), Diagnostic> {
    crate::language::limits::expression(&goal.expression)?;
    let branch = branch(machine, &goal.branch, "/goal/branch")?;
    finite(machine, branch, &goal.expression, "/goal/expression").map_err(planning_error)
}

pub(crate) fn operators(machine: &Machine, operators: &[Operator]) -> Result<(), Diagnostic> {
    let mut names = BTreeSet::new();
    for (index, operator) in operators.iter().enumerate() {
        let path = format!("/operators/{index}");
        if operator.name.is_empty()
            || !names.insert(&operator.name)
            || operator.operations.is_empty()
        {
            return Err(reason(
                "invalid_planning",
                "Operators require unique nonempty names and at least one branch operation",
                &path,
            ));
        }
        for (name, operation) in &operator.operations {
            let path = format!("{path}/operations/{}", key(name));
            branch(machine, name, &path)?;
            let index = machine.branch_id(name).expect("validated branch");
            if !machine.programs[index]
                .operations
                .iter()
                .any(|declaration| declaration.name == *operation)
            {
                return Err(reason(
                    "invalid_planning",
                    format!("Unknown operation {name}.{operation}"),
                    &path,
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn restrictions(machine: &Machine, operators: &[Operator]) -> Vec<Diagnostic> {
    let mut reasons = Vec::new();
    for (index, branch) in machine.document().branches.iter().enumerate() {
        for (operation_index, declaration) in machine.programs[index].operations.iter().enumerate()
        {
            if !operators
                .iter()
                .any(|operator| operator.operations.get(&branch.name) == Some(&declaration.name))
            {
                continue;
            }
            let path = operation_path(branch, index, operation_index);
            for (field, expression) in [
                ("guard", &declaration.guard),
                ("postcondition", &declaration.postcondition),
            ] {
                if let Some(expression) = expression {
                    inspect(
                        machine,
                        branch,
                        expression,
                        &format!("{path}/{field}"),
                        &mut reasons,
                    );
                }
            }
            for (update_index, update) in declaration.updates.iter().enumerate() {
                let path = format!("{path}/updates/{update_index}");
                if branch.data.iter().any(|data| data.name == update.field) {
                    reasons.push(reason(
                        "retained_data_update",
                        "Explored operations may update only finite facts",
                        &format!("{path}/field"),
                    ));
                }
                inspect(
                    machine,
                    branch,
                    &update.value,
                    &format!("{path}/value"),
                    &mut reasons,
                );
            }
        }
        let path = format!("/branches/{index}");
        if let Some(expression) = &branch.activating {
            inspect(
                machine,
                branch,
                expression,
                &format!("{path}/activating"),
                &mut reasons,
            );
        }
        for (i, fact) in branch.facts.iter().enumerate() {
            if let Some(expression) = &fact.derived {
                inspect(
                    machine,
                    branch,
                    expression,
                    &format!("{path}/facts/{i}/derived"),
                    &mut reasons,
                );
            }
        }
        for (field, predicates) in [
            ("labels", &branch.labels),
            ("named_states", &branch.named_states),
        ] {
            for (i, predicate) in predicates.iter().enumerate() {
                inspect(
                    machine,
                    branch,
                    &predicate.expression,
                    &format!("{path}/{field}/{i}/expression"),
                    &mut reasons,
                );
            }
        }
        for (i, invariant) in machine.programs[index].invariants.iter().enumerate() {
            let suffix = if i < branch.invariants.len() {
                format!("invariants/{i}/expression")
            } else {
                let (state, value) = branch
                    .states
                    .iter()
                    .enumerate()
                    .flat_map(|(s, state)| (0..state.values.len()).map(move |v| (s, v)))
                    .nth(i - branch.invariants.len())
                    .expect("lowered monitor invariant");
                format!("states/{state}/values/{value}")
            };
            inspect(
                machine,
                branch,
                &invariant.expression,
                &format!("{path}/{suffix}"),
                &mut reasons,
            );
        }
    }
    reasons
}

fn operation_path(branch: &Branch, index: usize, operation: usize) -> String {
    if operation < branch.operations.len() {
        format!("/branches/{index}/operations/{operation}")
    } else {
        let (state, transition) = branch
            .states
            .iter()
            .enumerate()
            .flat_map(|(s, state)| (0..state.transitions.len()).map(move |t| (s, t)))
            .nth(operation - branch.operations.len())
            .expect("lowered monitor operation");
        format!("/branches/{index}/states/{state}/transitions/{transition}")
    }
}

fn inspect(
    machine: &Machine,
    branch: &Branch,
    expression: &Expr,
    path: &str,
    reasons: &mut Vec<Diagnostic>,
) {
    if let Err(reason) = finite(machine, branch, expression, path) {
        reasons.push(reason);
    }
}

fn branch<'a>(machine: &'a Machine, name: &str, path: &str) -> Result<&'a Branch, Diagnostic> {
    machine
        .branch_id(name)
        .map(|index| &machine.document().branches[index])
        .ok_or_else(|| {
            reason(
                "invalid_planning",
                format!("Unknown planning branch {name:?}"),
                path,
            )
        })
}

fn finite(
    machine: &Machine,
    context: &Branch,
    expression: &Expr,
    path: &str,
) -> Result<(), Diagnostic> {
    match expression {
        Expr::Literal {
            value: Datum::Bool(_) | Datum::String(_) | Datum::Time(_),
        } => Ok(()),
        Expr::Read { source, field } => {
            let name = if source == "self" {
                context.name.as_str()
            } else {
                context
                    .reads
                    .iter()
                    .find(|read| read.alias == *source)
                    .map_or(source.as_str(), |read| read.branch.as_str())
            };
            if machine.branch_id(name).is_some_and(|index| {
                let target = &machine.document().branches[index];
                (field == "state" && !target.states.is_empty())
                    || target.facts.iter().any(|fact| fact.name == *field)
                    || target
                        .labels
                        .iter()
                        .chain(&target.named_states)
                        .any(|predicate| predicate.name == *field)
            }) {
                Ok(())
            } else {
                Err(reason(
                    "non_finite_read",
                    format!(
                        "Exploration requires a finite fact or predicate, found {source}.{field}"
                    ),
                    path,
                ))
            }
        }
        Expr::Not { expr } | Expr::IsUnknown { expr } => {
            finite(machine, context, expr, &format!("{path}/expr"))
        }
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
            finite(machine, context, left, &format!("{path}/left"))?;
            finite(machine, context, right, &format!("{path}/right"))
        }
        Expr::Literal { .. } => Err(reason(
            "non_finite_value",
            "Exploration requires known finite values",
            path,
        )),
    }
}

pub(crate) fn unknowns(state: &RuntimeState) -> Vec<Diagnostic> {
    let mut reasons = Vec::new();
    for (name, branch) in &state.branches {
        for (field, values) in [
            ("facts", &branch.facts),
            ("labels", &branch.labels),
            ("memberships", &branch.memberships),
        ] {
            for (name_of_value, value) in values {
                if matches!(value, Datum::Unknown(_)) {
                    reasons.push(reason(
                        "unknown_state",
                        "Finite exploration requires known facts and predicates",
                        &format!("/branches/{}/{field}/{}", key(name), key(name_of_value)),
                    ));
                }
            }
        }
    }
    reasons
}
