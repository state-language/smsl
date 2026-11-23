use std::collections::BTreeMap;

use crate::runtime::{RuntimeState, validation::error};
use crate::{
    Diagnostic, Machine,
    model::{Branch, Datum, Expr, Value},
};

use super::ConcreteInput;

pub(super) struct Condition<'a> {
    machine: &'a Machine,
    nodes: usize,
    inputs: BTreeMap<(String, String), ConcreteInput>,
}

impl<'a> Condition<'a> {
    pub fn new(machine: &'a Machine) -> Self {
        Self {
            machine,
            nodes: 0,
            inputs: BTreeMap::new(),
        }
    }

    pub fn inputs(self) -> Vec<ConcreteInput> {
        self.inputs.into_values().collect()
    }

    pub fn field(
        &mut self,
        branch: &'a Branch,
        name: &str,
        depth: usize,
    ) -> Result<Expr, Diagnostic> {
        self.reserve(depth)?;
        if let Some(binding) = branch
            .sub_branches
            .iter()
            .find(|binding| binding.fact == name)
        {
            let child = self.branch(&binding.branch);
            return self.expand(
                child,
                child.activating.as_ref().expect("compiled activation"),
                depth + 1,
            );
        }
        if let Some(expression) = branch
            .facts
            .iter()
            .find(|fact| fact.name == name)
            .and_then(|fact| fact.derived.as_ref())
        {
            return self.expand(branch, expression, depth + 1);
        }
        if let Some(label) = branch
            .labels
            .iter()
            .chain(&branch.named_states)
            .find(|label| label.name == name)
        {
            return self.expand(branch, &label.expression, depth + 1);
        }
        Ok(self.input(branch, name))
    }

    pub fn input(&mut self, branch: &Branch, field: &str) -> Expr {
        let range = branch
            .facts
            .iter()
            .find(|fact| fact.name == field)
            .and_then(|fact| fact.range);
        let values = if field == "state" {
            branch
                .states
                .iter()
                .map(|state| Value::String(state.name.clone()))
                .collect()
        } else {
            branch
                .facts
                .iter()
                .find(|fact| fact.name == field)
                .expect("compiled finite fact")
                .values
                .clone()
        };
        self.inputs
            .entry((branch.name.clone(), field.into()))
            .or_insert_with(|| ConcreteInput {
                branch: branch.name.clone(),
                field: field.into(),
                values,
                range,
            });
        Expr::Read {
            source: branch.name.clone(),
            field: field.into(),
        }
    }

    fn branch(&self, name: &str) -> &'a Branch {
        &self.machine.document().branches[self.machine.branch_id(name).expect("compiled branch")]
    }

    fn reserve(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.nodes += 1;
        if self.nodes > crate::limits::MAX_EXPRESSION_NODES
            || depth > crate::limits::MAX_EXPRESSION_DEPTH
        {
            return Err(error(
                "resource_limit",
                "Expanded concretization exceeds 64 levels or 4096 nodes",
            ));
        }
        Ok(())
    }

    fn expand(
        &mut self,
        branch: &'a Branch,
        expr: &'a Expr,
        depth: usize,
    ) -> Result<Expr, Diagnostic> {
        self.reserve(depth)?;
        Ok(match expr {
            Expr::Literal { value } => Expr::Literal {
                value: value.clone(),
            },
            Expr::Read { source, field } => {
                let target = if source == "self" {
                    branch
                } else {
                    self.branch(
                        branch
                            .reads
                            .iter()
                            .find(|read| read.alias == *source)
                            .map_or(source.as_str(), |read| read.branch.as_str()),
                    )
                };
                self.field(target, field, depth + 1)?
            }
            Expr::Not { expr } => Expr::Not {
                expr: Box::new(self.expand(branch, expr, depth + 1)?),
            },
            Expr::And { left, right } => Expr::And {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Or { left, right } => Expr::Or {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Eq { left, right } => Expr::Eq {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Ne { left, right } => Expr::Ne {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Lt { left, right } => Expr::Lt {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Le { left, right } => Expr::Le {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Gt { left, right } => Expr::Gt {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Ge { left, right } => Expr::Ge {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Add { left, right } => Expr::Add {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Sub { left, right } => Expr::Sub {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Mul { left, right } => Expr::Mul {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Div { left, right } => Expr::Div {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },
            Expr::Rem { left, right } => Expr::Rem {
                left: Box::new(self.expand(branch, left, depth + 1)?),
                right: Box::new(self.expand(branch, right, depth + 1)?),
            },

            _ => {
                return Err(error(
                    "invalid_abstraction",
                    "Concretization requires a pure finite mapping",
                ));
            }
        })
    }
}

pub(super) fn literal(value: &Value) -> Expr {
    Expr::Literal {
        value: match value {
            Value::Bool(value) => Datum::Bool(*value),
            Value::String(value) => Datum::String(value.clone()),
            Value::UInt(value) => Datum::Time(*value),
        },
    }
}

pub(super) fn equal(left: Expr, value: &Value) -> Expr {
    Expr::Eq {
        left: Box::new(left),
        right: Box::new(literal(value)),
    }
}

pub(super) fn conjunction(mut expressions: Vec<Expr>) -> Result<Expr, Diagnostic> {
    if expressions.is_empty() {
        return Ok(literal(&Value::Bool(true)));
    }
    while expressions.len() > 1 {
        let mut next = Vec::with_capacity(expressions.len().div_ceil(2));
        let mut iter = expressions.into_iter();
        while let Some(left) = iter.next() {
            next.push(match iter.next() {
                Some(right) => Expr::And {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                None => left,
            });
        }
        expressions = next;
    }
    crate::language::limits::checked_expression(expressions.pop().expect("nonempty conditions"))
}

pub(super) fn knowledge(condition: &mut Condition<'_>, state: &RuntimeState) -> Vec<Expr> {
    let mut result = Vec::new();
    for branch in &condition.machine.document().branches {
        let current = state.branch(&branch.name).expect("validated branch");
        if let Some(name) = &current.state {
            result.push(equal(
                condition.input(branch, "state"),
                &Value::String(name.clone()),
            ));
        }
        for fact in &branch.facts {
            if fact.derived.is_some()
                || branch
                    .sub_branches
                    .iter()
                    .any(|binding| binding.fact == fact.name)
            {
                continue;
            }
            let value = match &current.facts[&fact.name] {
                Datum::Bool(value) => Value::Bool(*value),
                Datum::String(value) => Value::String(value.clone()),
                Datum::Time(value) => Value::UInt(*value),
                Datum::Unknown(_) => continue,
                _ => unreachable!("validated fact"),
            };
            result.push(equal(condition.input(branch, &fact.name), &value));
        }
    }
    result
}
