use std::collections::HashMap;

use super::error;
use crate::diagnostic::{Diagnostic, Location};
use crate::language::model::{Branch, DataType, Datum, EventField, Expr, Fact, Value};
use crate::language::validation::unknown;

#[derive(Clone, Copy)]
pub(super) struct Type {
    pub kind: Option<DataType>,
    pub unknown: bool,
    pub nullable: bool,
    pub numeric_literal: bool,
}

impl Type {
    fn known(kind: DataType) -> Self {
        Self {
            kind: Some(kind),
            unknown: false,
            nullable: false,
            numeric_literal: false,
        }
    }

    pub fn accepts(self, kind: DataType) -> bool {
        self.kind == Some(kind)
            || (self.kind.is_none() && self.unknown)
            || self.numeric_literal && matches!(kind, DataType::Time | DataType::UInt)
    }

    fn compatible(self, other: Self) -> bool {
        self.kind.is_none()
            || other.kind.is_none()
            || self.kind == other.kind
            || self.numeric_literal && other.accepts(DataType::UInt)
            || other.numeric_literal && self.accepts(DataType::UInt)
    }
}

pub(super) struct Context<'a> {
    pub branch: &'a Branch,
    pub branches: &'a HashMap<&'a str, &'a Branch>,
    pub payload: Option<&'a [EventField]>,
    pub parameters: Option<&'a [EventField]>,
    pub constants: Option<&'a [EventField]>,
    pub event: bool,
    pub location: Option<Location>,
}

impl Context<'_> {
    pub fn infer(&self, expr: &Expr) -> Result<Type, Diagnostic> {
        let invalid = || {
            error(
                "invalid_expression",
                "Expression operands have incompatible types",
                self.location,
            )
        };
        let boolean = |kind: Type| kind.accepts(DataType::Bool);
        match expr {
            Expr::Literal {
                value: Datum::Event(_),
            } => Err(error(
                "invalid_expression",
                "Event references must be captured from events or reads",
                self.location,
            )),
            Expr::Literal { value } => Ok(Type {
                kind: value.data_type(),
                unknown: matches!(value, Datum::Unknown(_)),
                nullable: matches!(value, Datum::Null(())),
                numeric_literal: matches!(value, Datum::Time(_)),
            }),
            Expr::Read { source, field } => self.read_type(source, field),
            Expr::IsUnknown { expr } => {
                self.infer(expr)?;
                Ok(Type::known(DataType::Bool))
            }
            Expr::Not { expr } => {
                let value = self.infer(expr)?;
                if !boolean(value) {
                    return Err(invalid());
                }
                Ok(Type {
                    kind: Some(DataType::Bool),
                    unknown: value.unknown,
                    nullable: false,
                    numeric_literal: false,
                })
            }
            Expr::And { left, right } | Expr::Or { left, right } => {
                let left = self.infer(left)?;
                let right = self.infer(right)?;
                if !boolean(left) || !boolean(right) {
                    return Err(invalid());
                }
                Ok(Type {
                    kind: Some(DataType::Bool),
                    unknown: left.unknown || right.unknown,
                    nullable: false,
                    numeric_literal: false,
                })
            }
            Expr::Eq { left, right } | Expr::Ne { left, right } => {
                let left = self.infer(left)?;
                let right = self.infer(right)?;
                if !left.compatible(right) {
                    return Err(invalid());
                }
                Ok(Type {
                    kind: Some(DataType::Bool),
                    unknown: left.unknown || right.unknown,
                    nullable: false,
                    numeric_literal: false,
                })
            }
            Expr::Lt { left, right }
            | Expr::Le { left, right }
            | Expr::Gt { left, right }
            | Expr::Ge { left, right } => {
                let left = self.infer(left)?;
                let right = self.infer(right)?;
                if !left.compatible(right)
                    || ![left, right]
                        .iter()
                        .all(|kind| kind.accepts(DataType::Time) || kind.accepts(DataType::UInt))
                {
                    return Err(invalid());
                }
                Ok(Type {
                    kind: Some(DataType::Bool),
                    unknown: left.unknown || right.unknown,
                    nullable: false,
                    numeric_literal: false,
                })
            }
            Expr::Add { left, right }
            | Expr::Sub { left, right }
            | Expr::Mul { left, right }
            | Expr::Div { left, right }
            | Expr::Rem { left, right } => {
                let left = self.infer(left)?;
                let right = self.infer(right)?;
                if !left.accepts(DataType::UInt) || !right.accepts(DataType::UInt) {
                    return Err(invalid());
                }
                Ok(Type {
                    kind: Some(DataType::UInt),
                    unknown: left.unknown || right.unknown,
                    nullable: false,
                    numeric_literal: false,
                })
            }
        }
    }

    fn read_type(&self, source: &str, field: &str) -> Result<Type, Diagnostic> {
        if matches!(source, "payload" | "args") || source == "params" && self.constants.is_some() {
            let fields = match source {
                "payload" => self.payload,
                "params" => self.constants,
                _ => self.parameters,
            };
            return fields
                .and_then(|fields| fields.iter().find(|candidate| candidate.name == field))
                .map(|field| Type {
                    kind: Some(field.data_type),
                    unknown: false,
                    nullable: field.nullable,
                    numeric_literal: false,
                })
                .ok_or_else(|| unknown("parameter or payload field", field, self.location));
        }
        if source == "event" {
            if !self.event {
                return Err(error(
                    "invalid_expression",
                    "Event reads require an event reaction",
                    self.location,
                ));
            }
            let mut kind = Type::known(match field {
                "name" | "scope" | "subject" | "clock" => DataType::String,
                "time" => DataType::Time,
                "ref" | "cause" => DataType::Event,
                _ => return Err(unknown("event field", field, self.location)),
            });
            kind.nullable = field == "cause";
            return Ok(kind);
        }
        let target = resolve_branch(source, self.branch, self.branches)
            .ok_or_else(|| unknown("read branch or alias", source, self.location))?;
        if field == "state" {
            return Ok(Type {
                kind: Some(DataType::String),
                unknown: false,
                nullable: target.initial.is_none(),
                numeric_literal: false,
            });
        }
        if let Some(fact) = target.facts.iter().find(|fact| fact.name == field) {
            return Ok(Type {
                kind: Some(fact_type(fact)),
                unknown: true,
                nullable: false,
                numeric_literal: false,
            });
        }
        if target
            .labels
            .iter()
            .chain(&target.named_states)
            .any(|label| label.name == field)
        {
            return Ok(Type {
                kind: Some(DataType::Bool),
                unknown: true,
                nullable: false,
                numeric_literal: false,
            });
        }
        target
            .data
            .iter()
            .find(|data| data.name == field)
            .map(|data| Type {
                kind: Some(data.data_type),
                unknown: false,
                nullable: true,
                numeric_literal: false,
            })
            .ok_or_else(|| unknown("read field", field, self.location))
    }
}

pub(crate) fn resolve_branch<'a>(
    source: &str,
    branch: &'a Branch,
    branches: &HashMap<&str, &'a Branch>,
) -> Option<&'a Branch> {
    if source == "self" || source == branch.name {
        return Some(branch);
    }
    let name = branch
        .reads
        .iter()
        .find(|binding| binding.alias == source)
        .map(|binding| binding.branch.as_str())
        .unwrap_or(source);
    branches.get(name).copied()
}

pub(super) fn fact_type(fact: &Fact) -> DataType {
    if fact.range.is_some() {
        return DataType::UInt;
    }
    match fact.values[0] {
        Value::Bool(_) => DataType::Bool,
        Value::String(_) => DataType::String,
        Value::UInt(_) => DataType::UInt,
    }
}
