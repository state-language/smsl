use serde_json::{Map, Value};

use crate::{
    Diagnostic,
    source::lexer::{Kind, Token, tokenize},
};

pub(super) fn encode(kind: &str, value: &Value) -> Result<String, Diagnostic> {
    crate::encoding::validate_value(value)?;
    let source = format!("memory {kind} \"1\" {}\n", render(value, 0));
    crate::source::lexer::validate_source(&source)?;
    Ok(source)
}

fn render(value: &Value, depth: usize) -> String {
    if value == &serde_json::json!({"kind": "unknown"}) {
        return "unknown".into();
    }
    let indent = "  ".repeat(depth);
    let nested = "  ".repeat(depth + 1);
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    let name =
                        if name.bytes().enumerate().all(|(index, byte)| {
                            byte == b'_'
                                || byte.is_ascii_alphabetic()
                                || index > 0 && byte.is_ascii_digit()
                        }) && !matches!(name.as_str(), "true" | "false" | "null" | "unknown")
                            && !name.is_empty()
                        {
                            name.clone()
                        } else {
                            serde_json::to_string(name).expect("field name")
                        };
                    format!("{nested}{name}: {}", render(value, depth + 1))
                })
                .collect::<Vec<_>>()
                .join(",\n");
            format!("{{\n{fields}\n{indent}}}")
        }
        Value::Array(values) if !values.is_empty() => {
            let values = values
                .iter()
                .map(|value| format!("{nested}{}", render(value, depth + 1)))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[\n{values}\n{indent}]")
        }
        _ => serde_json::to_string(value).expect("validated value"),
    }
}

pub(super) fn decode(kind: &str, source: &str) -> Result<Value, Diagnostic> {
    let mut reader = Reader {
        tokens: tokenize(source)?,
        index: 0,
    };
    reader.expect(Kind::Name("memory".into()))?;
    reader.expect(Kind::Name(kind.into()))?;
    reader.expect(Kind::String("1".into()))?;
    let value = reader.value(0)?;
    reader.expect(Kind::End)?;
    crate::encoding::validate_value(&value)?;
    Ok(value)
}

struct Reader {
    tokens: Vec<Token>,
    index: usize,
}

impl Reader {
    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(
            "invalid_memory",
            message,
            Some(self.tokens[self.index].location),
        )
    }

    fn take(&mut self) -> Kind {
        let kind = self.tokens[self.index].kind.clone();
        if kind != Kind::End {
            self.index += 1;
        }
        kind
    }

    fn expect(&mut self, kind: Kind) -> Result<(), Diagnostic> {
        if self.tokens[self.index].kind != kind {
            return Err(self.error(format!("Expected {kind:?}")));
        }
        self.take();
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Value, Diagnostic> {
        if depth > crate::encoding::MAX_VALUE_DEPTH {
            return Err(self.error("Memory value exceeds the nesting limit"));
        }
        match self.take() {
            Kind::Null => Ok(Value::Null),
            Kind::Bool(value) => Ok(Value::Bool(value)),
            Kind::Number(value) => Ok(Value::from(value)),
            Kind::String(value) => Ok(Value::String(value)),
            Kind::Name(value) if value == "unknown" => Ok(serde_json::json!({"kind": "unknown"})),
            Kind::OpenBranch => {
                let mut fields = Map::new();
                if self.tokens[self.index].kind != Kind::CloseBranch {
                    loop {
                        let name = match self.take() {
                            Kind::Name(name) | Kind::String(name) => name,
                            _ => return Err(self.error("Expected a memory field name")),
                        };
                        self.expect(Kind::Colon)?;
                        let value = self.value(depth + 1)?;
                        if fields.insert(name.clone(), value).is_some() {
                            return Err(self.error(format!("Duplicate memory field {name:?}")));
                        }
                        if self.tokens[self.index].kind == Kind::CloseBranch {
                            break;
                        }
                        self.expect(Kind::Comma)?;
                    }
                }
                self.expect(Kind::CloseBranch)?;
                Ok(Value::Object(fields))
            }
            Kind::OpenList => {
                let mut values = Vec::new();
                if self.tokens[self.index].kind != Kind::CloseList {
                    loop {
                        values.push(self.value(depth + 1)?);
                        if self.tokens[self.index].kind == Kind::CloseList {
                            break;
                        }
                        self.expect(Kind::Comma)?;
                    }
                }
                self.expect(Kind::CloseList)?;
                Ok(Value::Array(values))
            }
            _ => Err(self.error("Expected a memory value")),
        }
    }
}
