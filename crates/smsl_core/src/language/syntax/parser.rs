mod declarations;
mod events;
mod expressions;
mod modules;
mod reactions;

use crate::diagnostic::{Diagnostic, Location};
use crate::language::model::{Document, Value};
use crate::language::modules::Module;
use crate::source::lexer::{Kind, Token, is_keyword, tokenize};

pub fn parse(source: &str) -> Result<Document, Diagnostic> {
    let module = parse_module(source)?;
    if !module.imports.is_empty() {
        return Err(Diagnostic::new(
            "unknown_reference",
            "Imports require a module resolver",
            None,
        ));
    }
    if !module.templates.is_empty()
        || !module.instances.is_empty()
        || !module.domains.is_empty()
        || module
            .branches
            .iter()
            .flat_map(|branch| &branch.facts)
            .any(|fact| fact.domain.is_some())
    {
        return crate::language::modules::link_document(
            crate::source::Source {
                id: "model.smsl".into(),
                text: source.into(),
                format: crate::source::SourceFormat::Smsl,
            },
            &mut |_: &str, _: &str| {
                Err(Diagnostic::new(
                    "unknown_reference",
                    "Imports require a module resolver",
                    None,
                ))
            },
        );
    }
    Ok(Document {
        version: module.version,
        events: module.events,
        branches: module.branches,
    })
}

pub fn parse_module(source: &str) -> Result<Module, Diagnostic> {
    Parser {
        tokens: tokenize(source)?,
        index: 0,
        expression_depth: 0,
        expression_nodes: 0,
    }
    .module()
}

pub fn parse_expression(source: &str) -> Result<crate::model::Expr, Diagnostic> {
    let mut parser = Parser {
        tokens: tokenize(source)?,
        index: 0,
        expression_depth: 0,
        expression_nodes: 0,
    };
    let expression = parser.expression()?;
    parser.expect(Kind::End)?;
    Ok(expression)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    expression_depth: usize,
    expression_nodes: usize,
}

impl Parser {
    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn take(&mut self) -> Token {
        let token = self.current().clone();
        if token.kind != Kind::End {
            self.index += 1;
        }
        token
    }

    fn consume(&mut self, kind: &Kind) -> bool {
        if self.current().kind == *kind {
            self.take();
            true
        } else {
            false
        }
    }

    fn keyword(&mut self, name: &str) -> bool {
        self.consume(&Kind::Name(name.into()))
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new("syntax", message, Some(self.current().location))
    }

    fn expect(&mut self, kind: Kind) -> Result<(), Diagnostic> {
        if self.consume(&kind) {
            Ok(())
        } else {
            Err(self.error(format!(
                "expected {kind:?}, found {:?}",
                self.current().kind
            )))
        }
    }

    fn name(&mut self) -> Result<(String, Location), Diagnostic> {
        match &self.current().kind {
            Kind::Name(value) if !is_keyword(value) => {
                let value = value.clone();
                Ok((value, self.take().location))
            }
            Kind::String(value) if !value.is_empty() => {
                let value = value.clone();
                Ok((value, self.take().location))
            }
            _ => Err(self.error("expected a nonreserved identifier or nonempty quoted name")),
        }
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        if let Kind::String(value) = self.current().kind.clone() {
            self.take();
            Ok(value)
        } else {
            Err(self.error("expected a quoted string"))
        }
    }

    fn value(&mut self) -> Result<Value, Diagnostic> {
        match self.current().kind.clone() {
            Kind::Bool(value) => {
                self.take();
                Ok(Value::Bool(value))
            }
            Kind::String(value) => {
                self.take();
                Ok(Value::String(value))
            }
            Kind::Number(value) => {
                self.take();
                Ok(Value::UInt(value))
            }
            _ => Err(self.error("expected a Boolean, quoted string or unsigned integer value")),
        }
    }

    fn unsigned(&mut self) -> Result<u64, Diagnostic> {
        if let Kind::Number(value) = self.current().kind {
            self.take();
            Ok(value)
        } else {
            Err(self.error("expected an unsigned integer"))
        }
    }

    fn list<T>(
        &mut self,
        close: Kind,
        mut item: impl FnMut(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<Vec<T>, Diagnostic> {
        let mut items = Vec::new();
        if self.consume(&close) {
            return Ok(items);
        }
        loop {
            items.push(item(self)?);
            if self.consume(&close) {
                return Ok(items);
            }
            self.expect(Kind::Comma)?;
            if self.consume(&close) {
                return Ok(items);
            }
        }
    }
}
