use super::Parser;
use crate::diagnostic::Diagnostic;
use crate::language::model::{DataField, Operation, Reaction, ReadBinding, Transition, Update};
use crate::source::lexer::Kind;

impl Parser {
    pub(super) fn data_field(&mut self) -> Result<DataField, Diagnostic> {
        let (name, location) = self.name()?;
        self.expect(Kind::Colon)?;
        let data_type = self.data_type()?;
        self.expect(Kind::Assign)?;
        let initial = self.datum()?;
        Ok(DataField {
            name,
            data_type,
            initial,
            location: Some(location),
        })
    }

    pub(super) fn read_binding(&mut self) -> Result<ReadBinding, Diagnostic> {
        let (alias, location) = self.name()?;
        self.expect(Kind::Colon)?;
        Ok(ReadBinding {
            alias,
            branch: self.name()?.0,
            location: Some(location),
        })
    }

    pub(super) fn entries(&mut self) -> Result<(Vec<Transition>, Vec<Reaction>), Diagnostic> {
        let mut transitions = Vec::new();
        let mut reactions = Vec::new();
        while !self.consume(&Kind::CloseBranch) {
            if self.keyword("on") {
                reactions.push(self.reaction()?);
            } else {
                let (operation, location) = self.name()?;
                self.expect(Kind::Arrow)?;
                let target = self.name()?.0;
                let updates = self.updates()?;
                self.expect(Kind::Semicolon)?;
                transitions.push(Transition {
                    operation,
                    target,
                    updates,
                    location: Some(location),
                });
            }
        }
        Ok((transitions, reactions))
    }

    pub(super) fn reaction(&mut self) -> Result<Reaction, Diagnostic> {
        let location = Some(self.current().location);
        let event = self.name()?.0;
        let guard = if self.keyword("when") {
            Some(self.expression()?)
        } else {
            None
        };
        let target = if self.consume(&Kind::Arrow) {
            Some(self.name()?.0)
        } else {
            None
        };
        let updates = self.updates()?;
        let emit = if self.keyword("emit") {
            self.expect(Kind::OpenMap)?;
            self.list(Kind::CloseMap, |parser| Ok(parser.name()?.0))?
        } else {
            Vec::new()
        };
        self.expect(Kind::Semicolon)?;
        Ok(Reaction {
            event,
            guard,
            target,
            updates,
            emit,
            location,
        })
    }

    pub(super) fn operation(&mut self) -> Result<Operation, Diagnostic> {
        let name = self.name()?.0;
        let guard = if self.keyword("when") {
            Some(self.expression()?)
        } else {
            None
        };
        let updates = self.updates()?;
        let postcondition = if self.keyword("ensure") {
            Some(self.expression()?)
        } else {
            None
        };
        Ok(Operation {
            name,
            guard,
            updates,
            postcondition,
        })
    }

    fn updates(&mut self) -> Result<Vec<Update>, Diagnostic> {
        if !self.keyword("set") {
            return Ok(Vec::new());
        }
        self.expect(Kind::OpenMap)?;
        self.list(Kind::CloseMap, |parser| {
            let field = parser.name()?.0;
            parser.expect(Kind::Assign)?;
            Ok(Update {
                field,
                value: parser.expression()?,
            })
        })
    }
}
