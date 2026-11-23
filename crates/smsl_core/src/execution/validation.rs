use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic,
    model::{Datum, Expr},
    runtime::World,
};

use super::{Block, Definition, error};

pub fn validate(definition: &Definition) -> Result<(), Diagnostic> {
    if definition.version != 1 {
        return Err(error("execution_version", "Unsupported execution version"));
    }
    if definition.id.is_empty() || definition.root.is_empty() || definition.blocks.is_empty() {
        return Err(error(
            "invalid_execution",
            "Execution identity, root and blocks are required",
        ));
    }
    if definition.blocks.len() > 4096 {
        return Err(error("resource_limit", "Execution exceeds 4096 blocks"));
    }
    let mut parents = BTreeMap::new();
    for (name, block) in &definition.blocks {
        let path = format!("/blocks/{}", crate::diagnostic::key(name));
        if name.is_empty() {
            return Err(
                error("invalid_execution", "Block identity must be nonempty").at_path(path),
            );
        }
        if let Block::Condition { expression, .. } = block {
            condition(expression).map_err(|cause| cause.prepend_path(&path))?;
        }
        match block {
            Block::Sequence { members, .. } | Block::FirstSuccess { members, .. }
                if members.is_empty() =>
            {
                return Err(
                    error("invalid_execution", "Composite members must be nonempty").at_path(path),
                );
            }
            Block::Parallel {
                members,
                success_threshold,
                failure_threshold,
            } => {
                let count = members.len() as u64;
                if *success_threshold == 0
                    || *failure_threshold == 0
                    || *success_threshold > count
                    || *failure_threshold > count
                    || success_threshold.checked_add(*failure_threshold) != count.checked_add(1)
                {
                    return Err(error("invalid_execution", "Parallel thresholds must be positive, within member count and sum to count + 1").at_path(path));
                }
            }
            Block::Operation {
                instance,
                operations,
            } if instance.is_empty()
                || operations.is_empty()
                || operations
                    .iter()
                    .any(|(branch, operation)| branch.is_empty() || operation.is_empty()) =>
            {
                return Err(error(
                    "invalid_execution",
                    "Operation requires an instance and nonempty branch operations",
                )
                .at_path(path));
            }
            Block::Condition {
                instance, branch, ..
            } if instance.is_empty() || branch.is_empty() => {
                return Err(error(
                    "invalid_execution",
                    "Condition requires an instance and branch",
                )
                .at_path(path));
            }
            Block::Action {
                instance,
                branch,
                action,
                success_outcomes,
                ..
            } => {
                let unique: BTreeSet<_> = success_outcomes.iter().collect();
                if instance.is_empty()
                    || branch.is_empty()
                    || action.is_empty()
                    || success_outcomes.is_empty()
                    || success_outcomes.iter().any(String::is_empty)
                    || unique.len() != success_outcomes.len()
                {
                    return Err(error(
                        "invalid_execution",
                        "Action requires an instance, branch, action and distinct success outcomes",
                    )
                    .at_path(path));
                }
            }
            Block::Scope {
                member,
                enter,
                exit,
                ..
            } => {
                if member.is_empty() {
                    return Err(
                        error("invalid_execution", "Scope requires a member").at_path(&path)
                    );
                }
                for hook in enter.iter().chain(exit.iter()) {
                    if !matches!(definition.blocks.get(hook), Some(Block::Operation { .. })) {
                        return Err(error(
                            "invalid_execution",
                            "Scope lifecycle hooks must name operation blocks",
                        )
                        .at_path(&path));
                    }
                }
            }
            _ => {}
        }
        for child in block.members() {
            if !definition.blocks.contains_key(&child) {
                return Err(
                    error("unknown_reference", format!("Unknown member {child:?}")).at_path(&path),
                );
            }
            if parents.insert(child, name.clone()).is_some() {
                return Err(error("invalid_execution", "Each block must have exactly one owner; instantiate reused definitions separately").at_path(&path));
            }
        }
    }
    if !definition.blocks.contains_key(&definition.root) || parents.contains_key(&definition.root) {
        return Err(error(
            "invalid_execution",
            "Root must name an unowned block",
        ));
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![(definition.root.clone(), 0)];
    while let Some((name, depth)) = pending.pop() {
        if depth >= 96 {
            return Err(error(
                "resource_limit",
                "Execution exceeds 96 nesting levels",
            ));
        }
        if !visited.insert(name.clone()) {
            return Err(error("invalid_execution", "Execution contains a cycle"));
        }
        pending.extend(
            definition.blocks[&name]
                .members()
                .into_iter()
                .map(|child| (child, depth + 1)),
        );
    }
    if visited.len() != definition.blocks.len() {
        return Err(error(
            "invalid_execution",
            "Every block must be reachable from the root",
        ));
    }
    Ok(())
}

fn condition(expression: &Expr) -> Result<(), Diagnostic> {
    crate::language::limits::expression(expression)?;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Expr::Literal {
                value: Datum::Event(_),
            } => {
                return Err(error(
                    "invalid_execution",
                    "Conditions cannot fabricate event references",
                ));
            }
            Expr::Read { source, .. }
                if matches!(source.as_str(), "event" | "payload" | "args") =>
            {
                return Err(error(
                    "invalid_execution",
                    "Conditions read model state, not event or action context",
                ));
            }
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
            | Expr::Rem { left, right } => {
                pending.push(left);
                pending.push(right);
            }
            Expr::Literal { .. } | Expr::Read { .. } => {}
        }
    }
    Ok(())
}

pub(crate) fn bindings(definition: &Definition, world: &World) -> Result<(), Diagnostic> {
    for (name, block) in &definition.blocks {
        let Some(instance) = block.instance() else {
            continue;
        };
        let machine = world.machine(instance).ok_or_else(|| {
            error(
                "unknown_reference",
                format!("Unknown model instance {instance:?}"),
            )
        })?;
        let path = format!("/blocks/{}", crate::diagnostic::key(name));
        match block {
            Block::Condition {
                branch, expression, ..
            } => {
                crate::language::validation::memory::state_predicate(
                    machine.document(),
                    branch,
                    expression,
                )
                .map_err(|cause| cause.prepend_path(&path))?;
            }
            Block::Operation { operations, .. } => {
                for (branch, operation) in operations {
                    let owner = machine
                        .document()
                        .branches
                        .iter()
                        .find(|item| item.name == *branch)
                        .ok_or_else(|| {
                            error(
                                "unknown_reference",
                                format!("Unknown operation branch {branch:?}"),
                            )
                            .at_path(&path)
                        })?;
                    if !owner.operations.iter().any(|item| item.name == *operation)
                        && !owner.states.iter().any(|state| {
                            state
                                .transitions
                                .iter()
                                .any(|item| item.operation == *operation)
                        })
                    {
                        return Err(error(
                            "unknown_reference",
                            format!("Unknown operation {branch}.{operation}"),
                        )
                        .at_path(&path));
                    }
                }
            }
            Block::Action {
                branch,
                action,
                arguments,
                success_outcomes,
                ..
            } => {
                let declared = machine
                    .document()
                    .branches
                    .iter()
                    .find(|item| item.name == *branch)
                    .and_then(|branch| branch.actions.iter().find(|item| item.name == *action))
                    .ok_or_else(|| {
                        error(
                            "unknown_reference",
                            format!("Unknown action {branch}.{action}"),
                        )
                        .at_path(&path)
                    })?;
                if success_outcomes
                    .iter()
                    .any(|name| !declared.outcomes.contains(name))
                {
                    return Err(error(
                        "unknown_reference",
                        "Success outcome is not declared by the action",
                    )
                    .at_path(&path));
                }
                crate::runtime::actions::arguments(
                    &declared.parameters,
                    arguments,
                    world.state(instance).expect("validated model instance"),
                    &|reference| world.resolve(reference).is_some(),
                )
                .map_err(|cause| cause.prepend_path(&path))?;
            }
            _ => {}
        }
    }
    Ok(())
}
