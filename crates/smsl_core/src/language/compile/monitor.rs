use crate::model::{Branch, Datum, Expr, Operation, Predicate, Reaction, Update, Value};

#[derive(Debug)]
pub(crate) struct Program {
    pub reactions: Vec<(Reaction, String)>,
    pub operations: Vec<Operation>,
    pub invariants: Vec<Predicate>,
}

fn mode(name: &str) -> Expr {
    Expr::Eq {
        left: Box::new(Expr::Read {
            source: "self".into(),
            field: "state".into(),
        }),
        right: Box::new(Expr::Literal {
            value: Datum::String(name.into()),
        }),
    }
}
fn guard(source: &str, condition: Option<Expr>) -> Expr {
    condition.map_or_else(
        || mode(source),
        |right| Expr::And {
            left: Box::new(mode(source)),
            right: Box::new(right),
        },
    )
}
fn lower_reaction(mut reaction: Reaction, source: Option<&str>) -> Reaction {
    if let Some(source) = source {
        reaction.guard = Some(guard(source, reaction.guard));
    }
    if let Some(target) = reaction.target.take() {
        reaction.updates.push(Update {
            field: "state".into(),
            value: Expr::Literal {
                value: Datum::String(target),
            },
        });
    }
    reaction
}

pub(crate) fn lower(branch: &Branch) -> Program {
    let mut program = Program {
        reactions: branch
            .reactions
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, r)| (lower_reaction(r, None), format!("/reactions/{index}")))
            .collect(),
        operations: branch.operations.clone(),
        invariants: branch.invariants.clone(),
    };
    for (state_index, state) in branch.states.iter().enumerate() {
        program.reactions.extend(
            state
                .reactions
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, r)| {
                    (
                        lower_reaction(r, Some(&state.name)),
                        format!("/states/{state_index}/reactions/{index}"),
                    )
                }),
        );
        for transition in &state.transitions {
            let mut updates = transition.updates.clone();
            updates.push(Update {
                field: "state".into(),
                value: Expr::Literal {
                    value: Datum::String(transition.target.clone()),
                },
            });
            program.operations.push(Operation {
                name: transition.operation.clone(),
                guard: Some(mode(&state.name)),
                updates,
                postcondition: None,
            });
        }
        for assignment in &state.values {
            let value = match &assignment.value {
                Value::Bool(v) => Datum::Bool(*v),
                Value::String(v) => Datum::String(v.clone()),
                Value::UInt(v) => Datum::Time(*v),
            };
            program.invariants.push(Predicate {
                name: format!("{} requires {}", state.name, assignment.fact),
                expression: Expr::Or {
                    left: Box::new(Expr::Not {
                        expr: Box::new(mode(&state.name)),
                    }),
                    right: Box::new(Expr::Eq {
                        left: Box::new(Expr::Read {
                            source: "self".into(),
                            field: assignment.fact.clone(),
                        }),
                        right: Box::new(Expr::Literal { value }),
                    }),
                },
            });
        }
    }
    program
}
