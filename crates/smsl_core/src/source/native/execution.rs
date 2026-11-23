use super::parser::{Object, Parser, Result, child};
use crate::source::lexer::Kind;
use serde_json::Value;

impl Parser {
    pub fn execution(&mut self) -> Result<Value> {
        let id = self.name("/id")?;
        self.expect(Kind::OpenBranch)?;
        let mut definition =
            Object::from_iter([("version".into(), 1.into()), ("id".into(), id.into())]);
        let mut blocks = Object::new();
        while !self.consume(Kind::CloseBranch) {
            let start = self.start();
            if self.word("version") {
                self.unique("/version")?;
                definition.insert("version".into(), self.number("/version")?.into());
                self.end()?;
            } else if self.word("initial") {
                self.field_name(&mut definition, "root", "")?;
                self.end()?;
            } else if self.word("block") {
                let name = self.name("")?;
                let path = child("/blocks", &name);
                self.unique(&path)?;
                self.expect(Kind::Colon)?;
                let mechanism = self.name(&child(&path, "mechanism"))?;
                self.expect(Kind::OpenBranch)?;
                let mut block = Object::from_iter([("mechanism".into(), mechanism.into())]);
                while !self.consume(Kind::CloseBranch) {
                    if self.word("input") {
                        let field = self.name("")?;
                        let field_path = child(&path, &field);
                        self.unique(&field_path)?;
                        if field == "mechanism" {
                            return Err(
                                self.error("The mechanism is declared after the block name")
                            );
                        }
                        self.expect(Kind::Assign)?;
                        block.insert(field, self.value(&field_path)?);
                        self.end()?;
                    } else if self.word("when") {
                        let field_path = child(&path, "expression");
                        self.unique(&field_path)?;
                        let origin = self.token().location;
                        let start = origin.offset;
                        while self.token().kind != Kind::Semicolon {
                            if matches!(self.token().kind, Kind::End | Kind::CloseBranch) {
                                return Err(self.error("Expected ';' after condition"));
                            }
                            self.take();
                        }
                        let expression = crate::language::syntax::parser::parse_expression(
                            &self.source[start..self.start()],
                        )
                        .map_err(|mut cause| {
                            if let Some(location) = &mut cause.location {
                                location.offset += origin.offset;
                                if location.line == 1 {
                                    location.column += origin.column - 1;
                                }
                                location.line += origin.line - 1;
                            }
                            cause.at_path(&field_path)
                        })?;
                        block.insert(
                            "expression".into(),
                            serde_json::to_value(expression).expect("expression record"),
                        );
                        self.end()?;
                        self.span(&field_path, start);
                    } else {
                        return Err(self.error("Expected input or when inside an execution block"));
                    }
                }
                self.span(&path, start);
                blocks.insert(name, block.into());
            } else {
                return Err(self.error("Expected version, initial or block"));
            }
        }
        definition.insert("blocks".into(), blocks.into());
        Ok(definition.into())
    }
}
