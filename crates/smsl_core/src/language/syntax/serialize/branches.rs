use super::{expression, expressions, identifier, parameters, value};
use crate::language::model::Branch;
use std::fmt::Write;

pub(super) fn write_branch(source: &mut String, branch: &Branch) {
    write_parameterized_branch(source, branch, None);
}

pub(super) fn write_parameterized_branch(
    source: &mut String,
    branch: &Branch,
    fields: Option<&[crate::model::EventField]>,
) {
    writeln!(
        source,
        "\nbranch {}{}{} {{",
        identifier(&branch.name),
        fields
            .map(|fields| format!("({})", parameters(fields)))
            .unwrap_or_default(),
        if branch.mutable { " mutable" } else { "" }
    )
    .unwrap();
    for fact in &branch.facts {
        let domain = fact
            .domain
            .as_ref()
            .map(|name| identifier(name))
            .unwrap_or_else(|| super::domain(&fact.values, fact.range));
        let (keyword, assigned) = if let Some(derived) = &fact.derived {
            ("derive", format!(" = {}", expression(derived)))
        } else {
            (
                "fact",
                fact.initial
                    .as_ref()
                    .map(|v| format!(" = {}", expressions::datum(v)))
                    .unwrap_or_default(),
            )
        };
        writeln!(
            source,
            "    {keyword} {}: {domain}{assigned};",
            identifier(&fact.name)
        )
        .unwrap();
    }
    for data in &branch.data {
        writeln!(
            source,
            "    data {}: {} = {};",
            identifier(&data.name),
            expressions::data_type(data.data_type),
            expressions::datum(&data.initial)
        )
        .unwrap();
    }
    for binding in &branch.reads {
        writeln!(
            source,
            "    read {}: {};",
            identifier(&binding.alias),
            identifier(&binding.branch)
        )
        .unwrap();
    }
    for binding in &branch.sub_branches {
        writeln!(
            source,
            "    child {}: {};",
            identifier(&binding.branch),
            identifier(&binding.fact)
        )
        .unwrap();
    }
    for label in &branch.labels {
        writeln!(
            source,
            "    label {} = {};",
            identifier(&label.name),
            expression(&label.expression)
        )
        .unwrap();
    }
    for (keyword, predicates) in [
        ("state", &branch.named_states),
        ("invariant", &branch.invariants),
    ] {
        for predicate in predicates {
            writeln!(
                source,
                "    {keyword} {} when {};",
                identifier(&predicate.name),
                expression(&predicate.expression)
            )
            .unwrap();
        }
    }
    for operation in &branch.operations {
        let guard = operation
            .guard
            .as_ref()
            .map(|e| format!(" when {}", expression(e)))
            .unwrap_or_default();
        let ensure = operation
            .postcondition
            .as_ref()
            .map(|e| format!(" ensure {}", expression(e)))
            .unwrap_or_default();
        writeln!(
            source,
            "    operation {}{guard}{}{ensure};",
            identifier(&operation.name),
            expressions::updates(&operation.updates)
        )
        .unwrap();
    }
    for abstraction in &branch.abstractions {
        let facts = abstraction
            .facts
            .iter()
            .map(|fact| identifier(fact))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            source,
            "    abstract {}({facts});",
            identifier(&abstraction.name)
        )
        .unwrap();
    }
    if let Some(initial) = &branch.initial {
        writeln!(source, "    initial {};", identifier(initial)).unwrap();
    }
    if let Some(activating) = &branch.activating {
        writeln!(source, "    activate {};", expression(activating)).unwrap();
    }
    for action in &branch.actions {
        let guard = action
            .precondition
            .as_ref()
            .map(|p| format!(" when {}", expression(p)))
            .unwrap_or_default();
        let outcomes = action
            .outcomes
            .iter()
            .map(|name| identifier(name))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            source,
            "    action {}({}){guard} outcomes({outcomes});",
            identifier(&action.name),
            parameters(&action.parameters)
        )
        .unwrap();
    }
    for reaction in &branch.reactions {
        writeln!(source, "    {};", expressions::reaction(reaction)).unwrap();
    }
    for state in &branch.states {
        let values = if state.values.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                state
                    .values
                    .iter()
                    .map(|a| format!("{}: {}", identifier(&a.fact), value(&a.value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        writeln!(source, "\n    state {}{values} {{", identifier(&state.name)).unwrap();
        for transition in &state.transitions {
            writeln!(
                source,
                "        {} -> {}{};",
                identifier(&transition.operation),
                identifier(&transition.target),
                expressions::updates(&transition.updates)
            )
            .unwrap();
        }
        for reaction in &state.reactions {
            writeln!(source, "        {};", expressions::reaction(reaction)).unwrap();
        }
        writeln!(source, "    }}").unwrap();
    }
    writeln!(source, "}}").unwrap();
}
