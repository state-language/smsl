use super::identifier;
use crate::language::model::{DataType, Datum, Expr, Reaction, Update};

pub(super) fn data_type(kind: DataType) -> &'static str {
    match kind {
        DataType::Bool => "bool",
        DataType::String => "string",
        DataType::Time => "time",
        DataType::Event => "event",
        DataType::UInt => "uint",
    }
}

pub(super) fn datum(value: &Datum) -> String {
    match value {
        Datum::Unknown(_) => "unknown".into(),
        Datum::Null(()) => "null".into(),
        Datum::Bool(value) => value.to_string(),
        Datum::String(value) => serde_json::to_string(value).expect("String is JSON serializable"),
        Datum::Time(value) => value.to_string(),
        Datum::Event(_) => unreachable!("Event reference literals are rejected during validation"),
    }
}

pub(crate) fn expression(expr: &Expr) -> String {
    match expr {
        Expr::Literal { value } => datum(value),
        Expr::Read { source, field } => format!(
            "{}.{}",
            if source == "event" {
                source.clone()
            } else {
                identifier(source)
            },
            if field == "state" || source == "event" && field == "time" {
                field.clone()
            } else {
                identifier(field)
            }
        ),
        Expr::IsUnknown { expr } => format!("is_unknown({})", expression(expr)),
        Expr::Not { expr } => format!("(not {})", expression(expr)),
        _ => {
            let (operator, left, right) = match expr {
                Expr::And { left, right } => ("and", left, right),
                Expr::Or { left, right } => ("or", left, right),
                Expr::Eq { left, right } => ("==", left, right),
                Expr::Ne { left, right } => ("!=", left, right),
                Expr::Lt { left, right } => ("<", left, right),
                Expr::Le { left, right } => ("<=", left, right),
                Expr::Gt { left, right } => (">", left, right),
                Expr::Ge { left, right } => (">=", left, right),
                Expr::Add { left, right } => ("+", left, right),
                Expr::Sub { left, right } => ("-", left, right),
                Expr::Mul { left, right } => ("*", left, right),
                Expr::Div { left, right } => ("/", left, right),
                Expr::Rem { left, right } => ("%", left, right),
                _ => unreachable!("handled nonbinary expression"),
            };
            format!("({} {operator} {})", expression(left), expression(right))
        }
    }
}

pub(super) fn reaction(reaction: &Reaction) -> String {
    let mut text = format!("on {}", identifier(&reaction.event));
    if let Some(guard) = &reaction.guard {
        text.push_str(&format!(" when {}", expression(guard)));
    }
    if let Some(target) = &reaction.target {
        text.push_str(&format!(" -> {}", identifier(target)));
    }
    text.push_str(&updates(&reaction.updates));
    if !reaction.emit.is_empty() {
        let emitted = reaction
            .emit
            .iter()
            .map(|name| identifier(name))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!(" emit({emitted})"));
    }
    text
}

pub(super) fn updates(updates: &[Update]) -> String {
    if updates.is_empty() {
        return String::new();
    }
    let values = updates
        .iter()
        .map(|update| {
            format!(
                "{} = {}",
                identifier(&update.field),
                expression(&update.value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(" set({values})")
}
