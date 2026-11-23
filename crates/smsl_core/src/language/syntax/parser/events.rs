use super::Parser;
use crate::diagnostic::Diagnostic;
use crate::language::model::{Action, DataType, EventDeclaration, EventField};
use crate::source::lexer::Kind;

impl Parser {
    pub(super) fn data_type(&mut self) -> Result<DataType, Diagnostic> {
        match self.take().kind {
            Kind::Name(name) if name == "bool" => Ok(DataType::Bool),
            Kind::Name(name) if name == "string" => Ok(DataType::String),
            Kind::Name(name) if name == "time" => Ok(DataType::Time),
            Kind::Name(name) if name == "event" => Ok(DataType::Event),
            Kind::Name(name) if name == "uint" => Ok(DataType::UInt),
            _ => Err(self.error("expected bool, string, time, event or uint type")),
        }
    }

    pub(super) fn event_declaration(&mut self) -> Result<EventDeclaration, Diagnostic> {
        let name = self.name()?.0;
        let fields = self.parameters()?;
        Ok(EventDeclaration { name, fields })
    }

    pub(super) fn parameters(&mut self) -> Result<Vec<EventField>, Diagnostic> {
        self.expect(Kind::OpenMap)?;
        self.list(Kind::CloseMap, |parser| {
            let name = parser.name()?.0;
            parser.expect(Kind::Colon)?;
            let data_type = parser.data_type()?;
            let nullable = parser.consume(&Kind::Question);
            Ok(EventField {
                name,
                data_type,
                nullable,
            })
        })
    }

    pub(super) fn action(&mut self) -> Result<Action, Diagnostic> {
        let name = self.name()?.0;
        let parameters = self.parameters()?;
        let precondition = if self.keyword("when") {
            Some(self.expression()?)
        } else {
            None
        };
        self.expect(Kind::Name("outcomes".into()))?;
        self.expect(Kind::OpenMap)?;
        let outcomes = self.list(Kind::CloseMap, |p| Ok(p.name()?.0))?;
        Ok(Action {
            name,
            parameters,
            precondition,
            outcomes,
        })
    }
}
