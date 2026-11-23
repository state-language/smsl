use crate::{
    Diagnostic,
    model::{Branch, Expr, Reaction},
};

pub(crate) fn visit(
    branch: &mut Branch,
    visitor: &mut impl FnMut(&mut Expr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    if let Some(expression) = &mut branch.activating {
        walk(expression, visitor)?;
    }
    for fact in &mut branch.facts {
        if let Some(expression) = &mut fact.derived {
            walk(expression, visitor)?;
        }
    }
    for label in branch
        .labels
        .iter_mut()
        .chain(&mut branch.named_states)
        .chain(&mut branch.invariants)
    {
        walk(&mut label.expression, visitor)?;
    }
    for operation in &mut branch.operations {
        for expression in operation
            .guard
            .iter_mut()
            .chain(operation.postcondition.iter_mut())
        {
            walk(expression, visitor)?;
        }
        for update in &mut operation.updates {
            walk(&mut update.value, visitor)?;
        }
    }
    for action in &mut branch.actions {
        if let Some(expression) = &mut action.precondition {
            walk(expression, visitor)?;
        }
    }
    for reaction in &mut branch.reactions {
        visit_reaction(reaction, visitor)?;
    }
    for state in &mut branch.states {
        for transition in &mut state.transitions {
            for update in &mut transition.updates {
                walk(&mut update.value, visitor)?;
            }
        }
        for reaction in &mut state.reactions {
            visit_reaction(reaction, visitor)?;
        }
    }
    Ok(())
}

fn visit_reaction(
    reaction: &mut Reaction,
    visitor: &mut impl FnMut(&mut Expr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    if let Some(expression) = &mut reaction.guard {
        walk(expression, visitor)?;
    }
    for update in &mut reaction.updates {
        walk(&mut update.value, visitor)?;
    }
    Ok(())
}

fn walk(
    expression: &mut Expr,
    visitor: &mut impl FnMut(&mut Expr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    visitor(expression)?;
    match expression {
        Expr::Not { expr } | Expr::IsUnknown { expr } => walk(expr, visitor)?,
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
            walk(left, visitor)?;
            walk(right, visitor)?;
        }
        Expr::Literal { .. } | Expr::Read { .. } => {}
    }
    Ok(())
}
