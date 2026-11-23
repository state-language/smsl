use super::ParsedDocument;
use crate::{
    Diagnostic,
    source::lexer::{Kind, Token, tokenize},
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Object = Map<String, Value>;
pub(super) type Result<T> = std::result::Result<T, Diagnostic>;

pub(super) struct Parser {
    pub source: String,
    pub tokens: Vec<Token>,
    pub index: usize,
    pub spans: BTreeMap<String, std::ops::Range<usize>>,
    seen: BTreeSet<String>,
}

impl Parser {
    pub fn new(source: &str) -> Result<Self> {
        Ok(Self {
            source: source.into(),
            tokens: tokenize(source)?,
            index: 0,
            spans: BTreeMap::new(),
            seen: BTreeSet::new(),
        })
    }
    pub fn document(mut self) -> Result<ParsedDocument> {
        self.expect_word("smsl")?;
        if self.string("/version")? != "0.2" {
            return Err(self.error("Unsupported SMSL syntax version"));
        }
        self.expect(Kind::Semicolon)?;
        self.spans.clear();
        let (kind, value) = if self.word("system") {
            ("system", self.system()?)
        } else if self.word("application") {
            ("application", self.application()?)
        } else if self.word("execution") {
            ("execution", self.execution()?)
        } else {
            return Err(self.error("Expected system, application or execution declaration"));
        };
        self.expect(Kind::End)?;
        crate::encoding::validate_value(&value)?;
        Ok(ParsedDocument {
            kind,
            value,
            spans: self.spans,
        })
    }
    pub fn token(&self) -> &Token {
        &self.tokens[self.index]
    }
    pub fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new("syntax", message, Some(self.token().location))
    }
    pub fn take(&mut self) -> Token {
        let token = self.token().clone();
        if token.kind != Kind::End {
            self.index += 1;
        }
        token
    }
    pub fn consume(&mut self, kind: Kind) -> bool {
        if self.token().kind == kind {
            self.take();
            true
        } else {
            false
        }
    }
    pub fn expect(&mut self, kind: Kind) -> Result<()> {
        if self.consume(kind.clone()) {
            Ok(())
        } else {
            Err(self.error(format!("Expected {kind:?}")))
        }
    }
    pub fn word(&mut self, word: &str) -> bool {
        self.consume(Kind::Name(word.into()))
    }
    pub fn expect_word(&mut self, word: &str) -> Result<()> {
        self.expect(Kind::Name(word.into()))
    }
    pub fn start(&self) -> usize {
        self.token().location.offset
    }
    pub fn span(&mut self, path: &str, start: usize) {
        self.spans
            .insert(path.into(), start..self.tokens[self.index - 1].end);
    }
    pub fn unique(&mut self, path: &str) -> Result<()> {
        if !self.seen.insert(path.into()) {
            return Err(self
                .error(format!("Duplicate declaration {path}"))
                .at_path(path));
        }
        Ok(())
    }
    pub fn name(&mut self, path: &str) -> Result<String> {
        let start = self.start();
        let quoted = matches!(self.token().kind, Kind::String(_));
        let mut value = match self.token().kind.clone() {
            Kind::Name(name) | Kind::String(name) if !name.is_empty() => name,
            _ => return Err(self.error("Expected a name or quoted nonempty string")),
        };
        self.take();
        while !quoted && self.consume(Kind::Dot) {
            let Kind::Name(part) = self.token().kind.clone() else {
                return Err(self.error("Expected a name after '.'"));
            };
            self.take();
            value.push('.');
            value.push_str(&part);
        }
        self.span(path, start);
        Ok(value)
    }
    pub fn string(&mut self, path: &str) -> Result<String> {
        let start = self.start();
        let Kind::String(value) = self.token().kind.clone() else {
            return Err(self.error("Expected a quoted string"));
        };
        self.take();
        self.span(path, start);
        Ok(value)
    }
    pub fn number(&mut self, path: &str) -> Result<u64> {
        let start = self.start();
        let Kind::Number(value) = self.token().kind else {
            return Err(self.error("Expected an unsigned integer"));
        };
        self.take();
        self.span(path, start);
        Ok(value)
    }
    pub fn end(&mut self) -> Result<()> {
        self.expect(Kind::Semicolon)
    }
    pub fn field_name(&mut self, object: &mut Object, field: &str, path: &str) -> Result<()> {
        let path = child(path, field);
        self.unique(&path)?;
        object.insert(field.into(), self.name(&path)?.into());
        Ok(())
    }
    pub fn field_choice(
        &mut self,
        object: &mut Object,
        field: &str,
        path: &str,
        choices: &[&str],
    ) -> Result<()> {
        let path = child(path, field);
        self.unique(&path)?;
        let start = self.start();
        let Kind::Name(value) = self.token().kind.clone() else {
            return Err(self
                .error(format!("Expected {}", choices.join(" or ")))
                .at_path(path));
        };
        if !choices.contains(&value.as_str()) {
            return Err(self
                .error(format!("Expected {}", choices.join(" or ")))
                .at_path(path));
        }
        self.take();
        self.span(&path, start);
        object.insert(field.into(), value.into());
        Ok(())
    }
    pub fn field_string(&mut self, object: &mut Object, field: &str, path: &str) -> Result<()> {
        let path = child(path, field);
        self.unique(&path)?;
        object.insert(field.into(), self.string(&path)?.into());
        Ok(())
    }
}

pub(super) fn child(parent: &str, key: &str) -> String {
    format!("{parent}/{}", crate::diagnostic::key(key))
}
