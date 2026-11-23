use super::model::{Datum, Document, Expr};
use crate::{
    Diagnostic,
    limits::{MAX_DECLARATIONS, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, MAX_INPUT_BYTES},
};
use std::io::{self, Write};

fn exceeded(message: &str) -> Diagnostic {
    Diagnostic::new("resource_limit", message, None)
}

pub(crate) fn expression(expr: &Expr) -> Result<(), Diagnostic> {
    let mut pending = vec![(expr, 1)];
    let mut count = 0;
    while let Some((expr, depth)) = pending.pop() {
        count += 1;
        if depth > MAX_EXPRESSION_DEPTH || count > MAX_EXPRESSION_NODES {
            return Err(exceeded("Expression exceeds 64 levels or 4096 nodes"));
        }
        match expr {
            Expr::Not { expr } | Expr::IsUnknown { expr } => pending.push((expr, depth + 1)),
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
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            Expr::Literal { .. } | Expr::Read { .. } => {}
        }
    }
    Ok(())
}

pub(crate) fn checked_expression(expr: Expr) -> Result<Expr, Diagnostic> {
    if let Err(error) = expression(&expr) {
        discard(expr);
        return Err(error);
    }
    Ok(expr)
}

pub(crate) fn document(document: &mut Document) -> Result<(), Diagnostic> {
    let result = check_document(document);
    if result.is_err() {
        for branch in &mut document.branches {
            if let Some(expr) = branch.activating.take() {
                discard(expr);
            }
            for fact in &mut branch.facts {
                if let Some(expr) = fact.derived.take() {
                    discard(expr);
                }
            }
            for label in branch
                .labels
                .iter_mut()
                .chain(&mut branch.named_states)
                .chain(&mut branch.invariants)
            {
                discard(take(&mut label.expression));
            }
            for operation in &mut branch.operations {
                for expr in operation
                    .guard
                    .take()
                    .into_iter()
                    .chain(operation.postcondition.take())
                {
                    discard(expr);
                }
                for update in &mut operation.updates {
                    discard(take(&mut update.value));
                }
            }
            for action in &mut branch.actions {
                if let Some(expr) = action.precondition.take() {
                    discard(expr);
                }
            }
            for reaction in branch.reactions.iter_mut().chain(
                branch
                    .states
                    .iter_mut()
                    .flat_map(|state| &mut state.reactions),
            ) {
                if let Some(expr) = reaction.guard.take() {
                    discard(expr);
                }
                for update in &mut reaction.updates {
                    discard(take(&mut update.value));
                }
            }
            for transition in branch
                .states
                .iter_mut()
                .flat_map(|state| &mut state.transitions)
            {
                for update in &mut transition.updates {
                    discard(take(&mut update.value));
                }
            }
        }
    }
    result
}

pub(crate) fn check_document(document: &Document) -> Result<(), Diagnostic> {
    let mut count = document.events.len() + document.branches.len();
    for event in &document.events {
        count += event.fields.len();
    }
    for (branch_index, branch) in document.branches.iter().enumerate() {
        count += branch.facts.len()
            + branch.data.len()
            + branch.reads.len()
            + branch.sub_branches.len()
            + branch.states.len()
            + branch.labels.len()
            + branch.named_states.len()
            + branch.invariants.len()
            + branch.operations.len()
            + branch.actions.len()
            + branch.abstractions.len()
            + branch.reactions.len();
        count += branch
            .abstractions
            .iter()
            .map(|abstraction| abstraction.facts.len())
            .sum::<usize>();
        for fact in &branch.facts {
            count += fact.values.len();
        }
        count += branch
            .operations
            .iter()
            .map(|operation| operation.updates.len())
            .sum::<usize>();
        for action in &branch.actions {
            count += action.parameters.len() + action.outcomes.len();
        }
        for state in &branch.states {
            count += state.values.len() + state.transitions.len() + state.reactions.len();
            for transition in &state.transitions {
                count += transition.updates.len();
            }
        }
        for reaction in branch
            .reactions
            .iter()
            .chain(branch.states.iter().flat_map(|state| &state.reactions))
        {
            count += reaction.updates.len() + reaction.emit.len();
        }
        for (path, expr) in expression_paths(branch) {
            expression(expr)
                .map_err(|error| error.at_path(format!("/branches/{branch_index}{path}")))?;
        }
    }
    if count > MAX_DECLARATIONS {
        return Err(exceeded("Document exceeds 100000 declarations"));
    }
    serde_json::to_writer_pretty(SizeLimit(1), document)
        .map_err(|_| exceeded("Encoded document exceeds 8 MiB"))?;
    crate::encoding::validate_value(&serde_json::to_value(document).expect("bounded definition"))
}

struct SizeLimit(usize);

impl Write for SizeLimit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        if self.0 > MAX_INPUT_BYTES {
            return Err(io::Error::other("document size limit"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn discard(expr: Expr) {
    let mut pending = vec![expr];
    while let Some(expr) = pending.pop() {
        match expr {
            Expr::Not { expr } | Expr::IsUnknown { expr } => pending.push(*expr),
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
                pending.push(*left);
                pending.push(*right);
            }
            Expr::Literal { .. } | Expr::Read { .. } => {}
        }
    }
}

fn take(expr: &mut Expr) -> Expr {
    std::mem::replace(
        expr,
        Expr::Literal {
            value: Datum::Null(()),
        },
    )
}

fn expression_paths(branch: &crate::language::model::Branch) -> Vec<(String, &Expr)> {
    let mut result = Vec::new();
    if let Some(expr) = &branch.activating {
        result.push(("/activating".into(), expr));
    }
    for (index, fact) in branch.facts.iter().enumerate() {
        if let Some(expr) = &fact.derived {
            result.push((format!("/facts/{index}/derived"), expr));
        }
    }
    for (index, label) in branch.labels.iter().enumerate() {
        result.push((format!("/labels/{index}/expression"), &label.expression));
    }
    for (group, predicates) in [
        ("named_states", &branch.named_states),
        ("invariants", &branch.invariants),
    ] {
        for (index, predicate) in predicates.iter().enumerate() {
            result.push((
                format!("/{group}/{index}/expression"),
                &predicate.expression,
            ));
        }
    }
    for (index, operation) in branch.operations.iter().enumerate() {
        for (field, expression) in [
            ("guard", &operation.guard),
            ("postcondition", &operation.postcondition),
        ] {
            if let Some(expr) = expression {
                result.push((format!("/operations/{index}/{field}"), expr));
            }
        }
        for (u, update) in operation.updates.iter().enumerate() {
            result.push((
                format!("/operations/{index}/updates/{u}/value"),
                &update.value,
            ));
        }
    }
    for (index, action) in branch.actions.iter().enumerate() {
        if let Some(expr) = &action.precondition {
            result.push((format!("/actions/{index}/precondition"), expr));
        }
    }
    let mut reactions: Vec<_> = branch
        .reactions
        .iter()
        .enumerate()
        .map(|(index, reaction)| (format!("/reactions/{index}"), reaction))
        .collect();
    for (state_index, state) in branch.states.iter().enumerate() {
        for (index, transition) in state.transitions.iter().enumerate() {
            for (update_index, update) in transition.updates.iter().enumerate() {
                result.push((
                    format!(
                        "/states/{state_index}/transitions/{index}/updates/{update_index}/value"
                    ),
                    &update.value,
                ));
            }
        }
        reactions.extend(state.reactions.iter().enumerate().map(|(index, reaction)| {
            (format!("/states/{state_index}/reactions/{index}"), reaction)
        }));
    }
    for (path, reaction) in reactions {
        if let Some(expr) = &reaction.guard {
            result.push((format!("{path}/guard"), expr));
        }
        for (index, update) in reaction.updates.iter().enumerate() {
            result.push((format!("{path}/updates/{index}/value"), &update.value));
        }
    }
    result
}
