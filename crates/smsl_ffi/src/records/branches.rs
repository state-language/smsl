use super::{
    expressions::{expression, optional},
    *,
};
use crate::boundary::*;
use smsl_core::{Diagnostic, model::*};

unsafe fn update(record: &SmslUpdateRecord) -> Result<Update, Diagnostic> {
    Ok(Update {
        field: unsafe { text(record.field)? }.into(),
        value: unsafe { expression(record.value)? },
    })
}

unsafe fn reaction(record: &SmslReactionRecord) -> Result<Reaction, Diagnostic> {
    unsafe {
        Ok(Reaction {
            event: text(record.event)?.into(),
            target: optional_text(record.target)?,
            guard: optional(record.guard)?,
            updates: collect(record.updates, record.update_count, |value| update(value))?,
            emit: collect(record.emit, record.emit_count, |value| {
                Ok(text(*value)?.into())
            })?,
            location: None,
        })
    }
}

unsafe fn state(record: &SmslStateRecord) -> Result<State, Diagnostic> {
    unsafe {
        Ok(State {
            name: text(record.name)?.into(),
            values: collect(record.values, record.value_count, |value| {
                Ok(Assignment {
                    fact: text(value.fact)?.into(),
                    value: super::value(value.value)?,
                    location: None,
                })
            })?,
            transitions: collect(record.transitions, record.transition_count, |value| {
                Ok(Transition {
                    operation: text(value.operation)?.into(),
                    target: text(value.target)?.into(),
                    updates: collect(value.updates, value.update_count, |value| update(value))?,
                    location: None,
                })
            })?,
            reactions: collect(record.reactions, record.reaction_count, |value| {
                reaction(value)
            })?,
            location: None,
        })
    }
}

pub(super) unsafe fn branch(record: &SmslBranchRecord) -> Result<Branch, Diagnostic> {
    unsafe {
        Ok(Branch {
            name: text(record.name)?.into(),
            initial: optional_text(record.initial)?,
            activating: optional(record.activating)?,
            mutable: boolean(record.mutable)?,
            facts: collect(record.facts, record.fact_count, |fact| {
                Ok(Fact {
                    domain: None,
                    name: text(fact.name)?.into(),
                    values: collect(fact.values, fact.value_count, |datum| value(*datum))?,
                    range: None,
                    initial: if fact.initial.is_null() {
                        None
                    } else {
                        Some(value(fact.initial)?)
                    },
                    derived: optional(fact.derived)?,
                    location: None,
                })
            })?,
            named_states: Vec::new(),
            invariants: Vec::new(),
            operations: Vec::new(),
            states: collect(record.states, record.state_count, |value| state(value))?,
            sub_branches: collect(record.bindings, record.binding_count, |value| {
                Ok(Binding {
                    branch: text(value.branch)?.into(),
                    fact: text(value.fact)?.into(),
                    location: None,
                })
            })?,
            data: collect(record.data, record.data_count, |field| {
                Ok(DataField {
                    name: text(field.name)?.into(),
                    data_type: data_type(field.data_type)?,
                    initial: value(field.initial)?,
                    location: None,
                })
            })?,
            reads: collect(record.reads, record.read_count, |value| {
                Ok(ReadBinding {
                    alias: text(value.alias)?.into(),
                    branch: text(value.branch)?.into(),
                    location: None,
                })
            })?,
            reactions: collect(record.reactions, record.reaction_count, |value| {
                reaction(value)
            })?,
            labels: collect(record.labels, record.label_count, |value| {
                Ok(Predicate {
                    name: text(value.name)?.into(),
                    expression: expression(value.expression)?,
                })
            })?,
            actions: collect(record.actions, record.action_count, |action| {
                Ok(Action {
                    name: text(action.name)?.into(),
                    parameters: collect(action.parameters, action.parameter_count, |field| {
                        event_field(field)
                    })?,
                    precondition: optional(action.precondition)?,
                    outcomes: collect(action.outcomes, action.outcome_count, |name| {
                        Ok(text(*name)?.into())
                    })?,
                })
            })?,
            abstractions: collect(record.abstractions, record.abstraction_count, |mapping| {
                Ok(Abstraction {
                    name: text(mapping.name)?.into(),
                    facts: collect(mapping.facts, mapping.fact_count, |name| {
                        Ok(text(*name)?.into())
                    })?,
                })
            })?,
            location: None,
        })
    }
}
