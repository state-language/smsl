mod declarations;
pub(crate) mod expressions;

use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, Location};
use crate::language::model::{
    Branch, DataType, Datum, Document, EventDeclaration, EventField, Expr, Reaction, Update,
};
use crate::language::validation::unknown;
use expressions::{Context, fact_type};

pub(crate) fn state_predicate(
    document: &Document,
    branch: &str,
    expression: &Expr,
) -> Result<(), Diagnostic> {
    let branches: HashMap<_, _> = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), branch))
        .collect();
    let branch = branches
        .get(branch)
        .ok_or_else(|| unknown("condition branch", branch, None))?;
    boolean(
        expression,
        &Context {
            branch,
            branches: &branches,
            payload: None,
            parameters: None,
            constants: None,
            event: false,
            location: None,
        },
    )
}

pub(crate) fn validate_memory(document: &Document) -> Result<(), Diagnostic> {
    let branches = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), branch))
        .collect();
    declarations::events(&document.events)?;
    let events = document
        .events
        .iter()
        .map(|event| (event.name.as_str(), event))
        .collect();
    for (index, branch) in document.branches.iter().enumerate() {
        declarations::branch(branch, &branches)
            .map_err(|error| error.prepend_path(&format!("/branches/{index}")))?;
    }
    for (index, branch) in document.branches.iter().enumerate() {
        validate_branch(branch, &branches, &events, None)
            .map_err(|error| error.prepend_path(&format!("/branches/{index}")))?;
    }
    Ok(())
}

pub(crate) fn validate_templates(
    document: &Document,
    templates: &[(Branch, &[EventField])],
) -> Result<(), Diagnostic> {
    let branches = document
        .branches
        .iter()
        .map(|branch| (branch.name.as_str(), branch))
        .collect();
    let events = document
        .events
        .iter()
        .map(|event| (event.name.as_str(), event))
        .collect();
    for (branch, constants) in templates {
        validate_branch(branch, &branches, &events, Some(constants)).map_err(|cause| {
            cause.prepend_path(&format!(
                "/instances/{}",
                crate::diagnostic::key(&branch.name)
            ))
        })?;
    }
    Ok(())
}

fn validate_branch(
    branch: &Branch,
    branches: &HashMap<&str, &Branch>,
    events: &HashMap<&str, &EventDeclaration>,
    constants: Option<&[EventField]>,
) -> Result<(), Diagnostic> {
    let context = Context {
        branch,
        branches,
        payload: None,
        parameters: None,
        constants,
        event: false,
        location: branch.location,
    };
    if let Some(activation) = &branch.activating {
        boolean(activation, &context).map_err(|error| error.at_path("/activating"))?;
    }
    for (index, fact) in branch.facts.iter().enumerate() {
        if let Some(expr) = &fact.derived {
            value_type(expr, fact_type(fact), true, &context)
                .map_err(|error| error.at_path(format!("/facts/{index}/derived")))?;
            literal_domain(expr, fact, branch.location)?;
        }
    }
    for (group, predicates) in [
        ("labels", &branch.labels),
        ("named_states", &branch.named_states),
        ("invariants", &branch.invariants),
    ] {
        for (index, predicate) in predicates.iter().enumerate() {
            boolean(&predicate.expression, &context)
                .map_err(|error| error.at_path(format!("/{group}/{index}/expression")))?;
        }
    }
    for (index, operation) in branch.operations.iter().enumerate() {
        for (field, expression) in [
            ("guard", &operation.guard),
            ("postcondition", &operation.postcondition),
        ] {
            if let Some(expression) = expression {
                boolean(expression, &context)
                    .map_err(|error| error.at_path(format!("/operations/{index}/{field}")))?;
            }
        }
        validate_updates(&operation.updates, &context)
            .map_err(|error| error.prepend_path(&format!("/operations/{index}")))?;
    }
    for (index, action) in branch.actions.iter().enumerate() {
        if let Some(precondition) = &action.precondition {
            let context = Context {
                parameters: Some(&action.parameters),
                ..context
            };
            boolean(precondition, &context)
                .map_err(|error| error.at_path(format!("/actions/{index}/precondition")))?;
        }
    }
    for (index, reaction) in branch.reactions.iter().enumerate() {
        validate_reaction(reaction, &context, events)
            .map_err(|error| error.prepend_path(&format!("/reactions/{index}")))?;
    }
    for (state_index, state) in branch.states.iter().enumerate() {
        for (index, transition) in state.transitions.iter().enumerate() {
            validate_updates(&transition.updates, &context).map_err(|error| {
                error.prepend_path(&format!("/states/{state_index}/transitions/{index}"))
            })?;
        }
        for (index, reaction) in state.reactions.iter().enumerate() {
            validate_reaction(reaction, &context, events).map_err(|error| {
                error.prepend_path(&format!("/states/{state_index}/reactions/{index}"))
            })?;
        }
    }
    Ok(())
}

fn validate_reaction(
    reaction: &Reaction,
    parent: &Context<'_>,
    events: &HashMap<&str, &EventDeclaration>,
) -> Result<(), Diagnostic> {
    let context = Context {
        branch: parent.branch,
        branches: parent.branches,
        payload: events
            .get(reaction.event.as_str())
            .map(|event| event.fields.as_slice()),
        parameters: None,
        constants: parent.constants,
        event: true,
        location: reaction.location,
    };
    if reaction.event.is_empty() {
        return Err(error(
            "invalid_reaction",
            "Reaction event cannot be empty",
            context.location,
        ));
    }
    if let Some(target) = &reaction.target {
        if !context
            .branch
            .states
            .iter()
            .any(|state| state.name == *target)
        {
            return Err(unknown("reaction target", target, context.location).at_path("/target"));
        }
    }
    if let Some(guard) = &reaction.guard {
        boolean(guard, &context).map_err(|error| error.prepend_path("/guard"))?;
    }
    validate_updates(&reaction.updates, &context)?;
    let mut emitted = HashSet::new();
    for (index, name) in reaction.emit.iter().enumerate() {
        if events
            .get(name.as_str())
            .is_some_and(|event| !event.fields.is_empty())
        {
            return Err(error(
                "invalid_reaction",
                "Emitted events cannot require payload fields",
                context.location,
            )
            .at_path(format!("/emit/{index}")));
        }
        if name.is_empty() || !emitted.insert(name) {
            return Err(error(
                "invalid_reaction",
                "Emitted event names must be nonempty and distinct",
                context.location,
            )
            .at_path(format!("/emit/{index}")));
        }
    }
    Ok(())
}

fn validate_updates(updates: &[Update], context: &Context<'_>) -> Result<(), Diagnostic> {
    let mut names = HashSet::new();
    for (index, update) in updates.iter().enumerate() {
        let path = format!("/updates/{index}");
        if !names.insert(&update.field) {
            return Err(error(
                "invalid_reaction",
                format!("Duplicate update for {:?}", update.field),
                context.location,
            )
            .at_path(path));
        }
        if let Some(fact) = context
            .branch
            .facts
            .iter()
            .find(|fact| fact.name == update.field)
        {
            if fact.derived.is_some()
                || context
                    .branch
                    .sub_branches
                    .iter()
                    .any(|binding| binding.fact == update.field)
            {
                return Err(error(
                    "read_only_field",
                    format!("Derived fact {:?} is read-only", update.field),
                    context.location,
                )
                .at_path(path));
            }
            value_type(&update.value, fact_type(fact), true, context)
                .map_err(|error| error.at_path(format!("{path}/value")))?;
            literal_domain(&update.value, fact, context.location)
                .map_err(|error| error.at_path(format!("{path}/value")))?;
        } else if let Some(field) = context
            .branch
            .data
            .iter()
            .find(|field| field.name == update.field)
        {
            value_type(&update.value, field.data_type, false, context)
                .map_err(|error| error.at_path(format!("{path}/value")))?;
        } else {
            return Err(error(
                "invalid_reaction",
                format!("Unknown writable field {:?}", update.field),
                context.location,
            )
            .at_path(format!("{path}/field")));
        }
    }
    Ok(())
}

fn boolean(expr: &Expr, context: &Context<'_>) -> Result<(), Diagnostic> {
    if !context.infer(expr)?.accepts(DataType::Bool) {
        return Err(error(
            "invalid_expression",
            "A predicate must be Boolean or unknown",
            context.location,
        ));
    }
    Ok(())
}

fn value_type(
    expr: &Expr,
    kind: DataType,
    fact: bool,
    context: &Context<'_>,
) -> Result<(), Diagnostic> {
    let inferred = context.infer(expr)?;
    let forbidden = matches!(
        expr,
        Expr::Literal {
            value: Datum::Null(())
        }
    ) && fact
        || matches!(
            expr,
            Expr::Literal {
                value: Datum::Unknown(_)
            }
        ) && !fact;
    if forbidden || inferred.kind.is_some() && !inferred.accepts(kind) {
        return Err(error(
            "invalid_expression",
            "Update or derivation has the wrong value type",
            context.location,
        ));
    }
    Ok(())
}

fn literal_domain(
    expr: &Expr,
    fact: &crate::model::Fact,
    location: Option<Location>,
) -> Result<(), Diagnostic> {
    if let Expr::Literal { value } = expr {
        if !fact.accepts(value) {
            return Err(error(
                "invalid_domain",
                "Fact value is outside its declared domain",
                location,
            ));
        }
    }
    Ok(())
}

fn error(code: &str, message: impl Into<String>, location: Option<Location>) -> Diagnostic {
    Diagnostic::new(code, message, location)
}
