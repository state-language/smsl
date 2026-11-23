use super::parser::{Object, Parser, Result, child};
use crate::source::lexer::Kind;
use serde_json::Value;

impl Parser {
    pub fn value(&mut self, path: &str) -> Result<Value> {
        let start = self.start();
        let location = self.token().location;
        let value = match self.take().kind {
            Kind::String(text) => text.into(),
            Kind::Number(number) => number.into(),
            Kind::Bool(value) => value.into(),
            Kind::Null => Value::Null,
            Kind::OpenList => {
                let mut values = Vec::new();
                if !self.consume(Kind::CloseList) {
                    loop {
                        values.push(self.value(&child(path, &values.len().to_string()))?);
                        if self.consume(Kind::CloseList) {
                            break;
                        }
                        self.expect(Kind::Comma)?;
                    }
                }
                Value::Array(values)
            }
            Kind::OpenBranch => {
                let mut values = Object::new();
                while !self.consume(Kind::CloseBranch) {
                    let key = if matches!(self.token().kind, Kind::String(_)) {
                        self.string("")?
                    } else {
                        self.name("")?
                    };
                    let field = child(path, &key);
                    if values.contains_key(&key) {
                        return Err(self
                            .error(format!("Duplicate data key {key:?}"))
                            .at_path(field));
                    }
                    self.expect(Kind::Assign)?;
                    values.insert(key, self.value(&field)?);
                    self.end()?;
                }
                Value::Object(values)
            }
            _ => {
                return Err(crate::Diagnostic::new(
                    "syntax",
                    "Expected a string, unsigned integer, boolean, null, array or object value",
                    Some(location),
                )
                .at_path(path));
            }
        };
        self.span(path, start);
        Ok(value)
    }
}
