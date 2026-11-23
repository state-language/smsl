use super::Parser;
use crate::diagnostic::Diagnostic;
use crate::language::model::{
    Abstraction, Assignment, Binding, Branch, Fact, Predicate, State, UIntRange,
};
use crate::source::lexer::Kind;

impl Parser {
    pub(super) fn branch_body(
        &mut self,
        name: String,
        location: crate::diagnostic::Location,
    ) -> Result<Branch, Diagnostic> {
        let mutable = self.keyword("mutable");
        self.expect(Kind::OpenBranch)?;
        let mut branch = Branch {
            name,
            initial: None,
            activating: None,
            mutable,
            facts: Vec::new(),
            sub_branches: Vec::new(),
            data: Vec::new(),
            reads: Vec::new(),
            states: Vec::new(),
            named_states: Vec::new(),
            invariants: Vec::new(),
            operations: Vec::new(),
            reactions: Vec::new(),
            labels: Vec::new(),
            abstractions: Vec::new(),
            actions: Vec::new(),
            location: Some(location),
        };
        while !self.consume(&Kind::CloseBranch) {
            if self.keyword("state") {
                let (name, location) = self.name()?;
                if self.keyword("when") {
                    branch.named_states.push(Predicate {
                        name,
                        expression: self.expression()?,
                    });
                    self.expect(Kind::Semicolon)?;
                } else {
                    branch.states.push(self.state(name, location)?);
                }
                continue;
            }
            if self.keyword("on") {
                branch.reactions.push(self.reaction()?);
                continue;
            }
            let token = self.take();
            let Kind::Name(member) = token.kind else {
                return Err(self.error("expected a branch declaration"));
            };
            match member.as_str() {
                "fact" => branch.facts.push(self.fact(false)?),
                "derive" => branch.facts.push(self.fact(true)?),
                "data" => branch.data.push(self.data_field()?),
                "read" => branch.reads.push(self.read_binding()?),
                "child" => branch.sub_branches.push(self.binding()?),
                "action" => branch.actions.push(self.action()?),
                "operation" => branch.operations.push(self.operation()?),
                "invariant" => {
                    let name = self.name()?.0;
                    if !self.keyword("when") {
                        return Err(self.error("expected when"));
                    }
                    branch.invariants.push(Predicate {
                        name,
                        expression: self.expression()?,
                    });
                }
                "abstract" => {
                    let name = self.name()?.0;
                    self.expect(Kind::OpenMap)?;
                    let facts = self.list(Kind::CloseMap, |parser| Ok(parser.name()?.0))?;
                    branch.abstractions.push(Abstraction { name, facts });
                }
                "label" => {
                    let name = self.name()?.0;
                    self.expect(Kind::Assign)?;
                    branch.labels.push(Predicate {
                        name,
                        expression: self.expression()?,
                    });
                }
                "initial" => {
                    if branch.initial.is_some() {
                        return Err(Diagnostic::new(
                            "duplicate_name",
                            "duplicate initial declaration",
                            Some(token.location),
                        ));
                    }
                    branch.initial = Some(self.name()?.0);
                }
                "activate" => {
                    if branch.activating.is_some() {
                        return Err(Diagnostic::new(
                            "duplicate_name",
                            "duplicate activate declaration",
                            Some(token.location),
                        ));
                    }
                    branch.activating = Some(self.expression()?);
                }
                _ => {
                    return Err(Diagnostic::new(
                        "syntax",
                        format!("unknown branch declaration {member}"),
                        Some(token.location),
                    ));
                }
            }
            self.expect(Kind::Semicolon)?;
        }
        Ok(branch)
    }

    fn fact(&mut self, is_derived: bool) -> Result<Fact, Diagnostic> {
        let (name, location) = self.name()?;
        self.expect(Kind::Colon)?;
        let (values, range, domain) = if self.current().kind == Kind::OpenList
            || (self.current().kind == Kind::Name("uint".into())
                && self
                    .tokens
                    .get(self.index + 1)
                    .is_some_and(|token| token.kind == Kind::OpenList))
        {
            let (values, range) = self.domain_literal()?;
            (values, range, None)
        } else {
            (Vec::new(), None, Some(self.name()?.0))
        };
        let (initial, derived) = if is_derived {
            self.expect(Kind::Assign)?;
            (None, Some(self.expression()?))
        } else if self.consume(&Kind::Assign) {
            (Some(self.datum()?), None)
        } else {
            (None, None)
        };
        Ok(Fact {
            name,
            values,
            range,
            domain,
            initial,
            derived,
            location: Some(location),
        })
    }

    pub(super) fn domain_literal(
        &mut self,
    ) -> Result<(Vec<crate::model::Value>, Option<UIntRange>), Diagnostic> {
        Ok(if self.keyword("uint") {
            self.expect(Kind::OpenList)?;
            let min = self.unsigned()?;
            self.expect(Kind::Range)?;
            let max = self.unsigned()?;
            self.expect(Kind::CloseList)?;
            (Vec::new(), Some(UIntRange { min, max }))
        } else {
            self.expect(Kind::OpenList)?;
            (self.list(Kind::CloseList, Self::value)?, None)
        })
    }

    fn binding(&mut self) -> Result<Binding, Diagnostic> {
        let (branch, location) = self.name()?;
        self.expect(Kind::Colon)?;
        Ok(Binding {
            branch,
            fact: self.name()?.0,
            location: Some(location),
        })
    }

    fn state(
        &mut self,
        name: String,
        location: crate::diagnostic::Location,
    ) -> Result<State, Diagnostic> {
        let values = if self.consume(&Kind::OpenMap) {
            self.list(Kind::CloseMap, Self::assignment)?
        } else {
            Vec::new()
        };
        self.expect(Kind::OpenBranch)?;
        let (transitions, reactions) = self.entries()?;
        Ok(State {
            name,
            values,
            transitions,
            reactions,
            location: Some(location),
        })
    }

    fn assignment(&mut self) -> Result<Assignment, Diagnostic> {
        let (fact, location) = self.name()?;
        self.expect(Kind::Colon)?;
        Ok(Assignment {
            fact,
            value: self.value()?,
            location: Some(location),
        })
    }
}
