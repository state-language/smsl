use std::collections::BTreeMap;

use crate::{
    Diagnostic,
    language::model::{Branch, Datum, EventRef, Expr},
};

use super::{BranchValues, Event, error};

pub(crate) struct Evaluation<'a> {
    pub branches: &'a BranchValues,
    pub branch: &'a Branch,
    pub event: Option<&'a Event>,
    pub event_ref: Option<&'a EventRef>,
    pub args: Option<&'a BTreeMap<String, Datum>>,
    pub resolved_reads: bool,
}

impl<'a> Evaluation<'a> {
    pub fn state(branches: &'a BranchValues, branch: &'a Branch) -> Self {
        Self {
            branches,
            branch,
            event: None,
            event_ref: None,
            args: None,
            resolved_reads: false,
        }
    }

    pub fn resolved(branches: &'a BranchValues, branch: &'a Branch) -> Self {
        Self {
            resolved_reads: true,
            ..Self::state(branches, branch)
        }
    }

    pub fn boolean(&self, expression: &Expr) -> Result<bool, Diagnostic> {
        Ok(self.logical(expression)? == Some(true))
    }

    pub fn predicate(&self, expression: &Expr) -> Result<Datum, Diagnostic> {
        Ok(self
            .logical(expression)?
            .map_or_else(Datum::unknown, Datum::Bool))
    }

    pub(super) fn logical(&self, expression: &Expr) -> Result<Option<bool>, Diagnostic> {
        match self.evaluate(expression)? {
            Datum::Bool(value) => Ok(Some(value)),
            Datum::Unknown(_) => Ok(None),
            Datum::Null(()) => Err(error("unset_data", "A Boolean expression read unset data")),
            _ => Err(error("invalid_expression", "Expected a Boolean expression")),
        }
    }

    pub fn evaluate(&self, expression: &Expr) -> Result<Datum, Diagnostic> {
        match expression {
            Expr::Literal { value } => Ok(value.clone()),
            Expr::Read { source, field } => self.read(source, field),
            Expr::IsUnknown { expr } => Ok(Datum::Bool(matches!(
                self.evaluate(expr)?,
                Datum::Unknown(_)
            ))),
            Expr::Not { expr } => Ok(self
                .logical(expr)?
                .map_or_else(Datum::unknown, |value| Datum::Bool(!value))),
            Expr::And { left, right } => {
                let left = self.logical(left)?;
                if left == Some(false) {
                    return Ok(Datum::Bool(false));
                }
                let right = self.logical(right)?;
                Ok(match (left, right) {
                    (_, Some(false)) => Datum::Bool(false),
                    (Some(true), Some(true)) => Datum::Bool(true),
                    _ => Datum::unknown(),
                })
            }
            Expr::Or { left, right } => {
                let left = self.logical(left)?;
                if left == Some(true) {
                    return Ok(Datum::Bool(true));
                }
                let right = self.logical(right)?;
                Ok(match (left, right) {
                    (_, Some(true)) => Datum::Bool(true),
                    (Some(false), Some(false)) => Datum::Bool(false),
                    _ => Datum::unknown(),
                })
            }
            Expr::Eq { left, right } | Expr::Ne { left, right } => {
                let left = self.evaluate(left)?;
                let right = self.evaluate(right)?;
                if matches!(left, Datum::Unknown(_)) || matches!(right, Datum::Unknown(_)) {
                    return Ok(Datum::unknown());
                }
                Ok(Datum::Bool(
                    (left == right) == matches!(expression, Expr::Eq { .. }),
                ))
            }
            Expr::Lt { left, right }
            | Expr::Le { left, right }
            | Expr::Gt { left, right }
            | Expr::Ge { left, right } => {
                let left = self.evaluate(left)?;
                let right = self.evaluate(right)?;
                if matches!(left, Datum::Unknown(_)) || matches!(right, Datum::Unknown(_)) {
                    return Ok(Datum::unknown());
                }
                let (Datum::Time(left), Datum::Time(right)) = (left, right) else {
                    return Err(error(
                        "unset_data",
                        "An ordered comparison requires set numeric values",
                    ));
                };
                Ok(Datum::Bool(match expression {
                    Expr::Lt { .. } => left < right,
                    Expr::Le { .. } => left <= right,
                    Expr::Gt { .. } => left > right,
                    Expr::Ge { .. } => left >= right,
                    _ => unreachable!(),
                }))
            }
            Expr::Add { left, right }
            | Expr::Sub { left, right }
            | Expr::Mul { left, right }
            | Expr::Div { left, right }
            | Expr::Rem { left, right } => {
                let left = self.evaluate(left)?;
                let right = self.evaluate(right)?;
                if matches!(left, Datum::Null(())) || matches!(right, Datum::Null(())) {
                    return Err(error(
                        "unset_data",
                        "Arithmetic requires set unsigned values",
                    ));
                }
                if matches!(expression, Expr::Div { .. } | Expr::Rem { .. })
                    && matches!(right, Datum::Time(0))
                {
                    return Err(error(
                        "division_by_zero",
                        "Unsigned divisor must be nonzero",
                    ));
                }
                if matches!(left, Datum::Unknown(_)) || matches!(right, Datum::Unknown(_)) {
                    return Ok(Datum::unknown());
                }
                let (Datum::Time(left), Datum::Time(right)) = (left, right) else {
                    return Err(error(
                        "invalid_expression",
                        "Arithmetic requires unsigned values",
                    ));
                };
                let result = match expression {
                    Expr::Add { .. } => left.checked_add(right),
                    Expr::Sub { .. } => left.checked_sub(right),
                    Expr::Mul { .. } => left.checked_mul(right),
                    Expr::Div { .. } => left.checked_div(right),
                    Expr::Rem { .. } => left.checked_rem(right),
                    _ => unreachable!(),
                };
                result.map(Datum::Time).ok_or_else(|| {
                    error(
                        "arithmetic_overflow",
                        "Unsigned arithmetic is outside the u64 range",
                    )
                })
            }
        }
    }

    fn read(&self, source: &str, field: &str) -> Result<Datum, Diagnostic> {
        if self.resolved_reads {
            return self.branch_read(source, field);
        }
        let missing = || {
            error(
                "unknown_reference",
                format!("Unknown runtime field {source}.{field}"),
            )
        };
        if source == "args" {
            return self
                .args
                .and_then(|args| args.get(field))
                .cloned()
                .ok_or_else(missing);
        }
        if source == "payload" {
            return self
                .event
                .and_then(|event| event.payload.get(field))
                .cloned()
                .ok_or_else(missing);
        }
        if source == "event" {
            let event = self.event.ok_or_else(missing)?;
            return Ok(match field {
                "name" => Datum::String(event.name.clone()),
                "time" => Datum::Time(event.time),
                "ref" => Datum::Event(self.event_ref.ok_or_else(missing)?.clone()),
                "cause" => event.cause.clone().map_or(Datum::Null(()), Datum::Event),
                "scope" => Datum::String(event.context.scope.clone()),
                "subject" => Datum::String(event.context.subject.clone()),
                "clock" => Datum::String(event.context.clock.clone()),
                _ => return Err(missing()),
            });
        }
        let name = if source == "self" {
            self.branch.name.as_str()
        } else {
            self.branch
                .reads
                .iter()
                .find(|read| read.alias == source)
                .map_or(source, |read| read.branch.as_str())
        };
        self.branch_read(name, field)
    }

    fn branch_read(&self, name: &str, field: &str) -> Result<Datum, Diagnostic> {
        let missing = || {
            error(
                "unknown_reference",
                format!("Unknown runtime field {name}.{field}"),
            )
        };
        let current = self.branches.get(name).ok_or_else(missing)?;
        if field == "state" {
            return Ok(current
                .facts
                .get("state")
                .cloned()
                .unwrap_or(Datum::Null(())));
        }
        current
            .facts
            .get(field)
            .or_else(|| current.data.get(field))
            .or_else(|| current.labels.get(field))
            .or_else(|| current.memberships.get(field))
            .cloned()
            .ok_or_else(missing)
    }
}
