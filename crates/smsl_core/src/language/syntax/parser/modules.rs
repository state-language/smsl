use super::Parser;
use crate::diagnostic::Diagnostic;
use crate::language::modules::{Domain, Import, ImportedName, Instance, Module, Template};
use crate::source::lexer::Kind;
use std::collections::BTreeMap;

impl Parser {
    pub(super) fn module(mut self) -> Result<Module, Diagnostic> {
        self.expect(Kind::Name("smsl".into()))?;
        let version = self.string()?;
        self.expect(Kind::Semicolon)?;
        let mut module = Module {
            version,
            imports: Vec::new(),
            exports: Vec::new(),
            events: Vec::new(),
            branches: Vec::new(),
            templates: Vec::new(),
            instances: Vec::new(),
            domains: Vec::new(),
        };
        let mut exported = false;
        while self.current().kind != Kind::End {
            if self.keyword("import") {
                module.imports.push(self.import()?);
            } else if self.keyword("export") {
                if exported {
                    return Err(Diagnostic::new(
                        "duplicate_name",
                        "duplicate export declaration",
                        Some(self.current().location),
                    ));
                }
                exported = true;
                self.expect(Kind::OpenBranch)?;
                module.exports = self.list(Kind::CloseBranch, |p| Ok(p.name()?.0))?;
                self.expect(Kind::Semicolon)?;
            } else if self.keyword("event") {
                module.events.push(self.event_declaration()?);
                self.expect(Kind::Semicolon)?;
            } else if self.keyword("domain") {
                let name = self.name()?.0;
                self.expect(Kind::Assign)?;
                let (values, range) = self.domain_literal()?;
                self.expect(Kind::Semicolon)?;
                module.domains.push(Domain {
                    name,
                    values,
                    range,
                });
            } else if self.keyword("branch") {
                let (name, location) = self.name()?;
                if self.current().kind == Kind::OpenMap {
                    let parameters = self.parameters()?;
                    let branch = self.branch_body(name, location)?;
                    module.templates.push(Template { parameters, branch });
                } else {
                    module.branches.push(self.branch_body(name, location)?);
                }
            } else if self.keyword("instance") {
                module.instances.push(self.instance()?);
            } else {
                return Err(self.error(
                    "expected import, export, event, domain, branch, or instance declaration",
                ));
            }
        }
        Ok(module)
    }

    fn instance(&mut self) -> Result<Instance, Diagnostic> {
        let name = self.name()?.0;
        self.expect(Kind::Colon)?;
        let template = self.name()?.0;
        self.expect(Kind::OpenMap)?;
        let entries = self.list(Kind::CloseMap, |parser| {
            let key = parser.name()?.0;
            parser.expect(Kind::Assign)?;
            Ok((key, parser.datum()?))
        })?;
        self.expect(Kind::Semicolon)?;
        let mut arguments = BTreeMap::new();
        for (key, value) in entries {
            if arguments.insert(key.clone(), value).is_some() {
                return Err(self.error(format!("duplicate argument {key:?}")));
            }
        }
        Ok(Instance {
            name,
            template,
            arguments,
        })
    }

    fn import(&mut self) -> Result<Import, Diagnostic> {
        self.expect(Kind::OpenBranch)?;
        let names = self.list(Kind::CloseBranch, |p| {
            let name = p.name()?.0;
            let alias = if p.keyword("as") {
                p.name()?.0
            } else {
                name.clone()
            };
            Ok(ImportedName { name, alias })
        })?;
        self.expect(Kind::Name("from".into()))?;
        let source = self.string()?;
        self.expect(Kind::Semicolon)?;
        Ok(Import { source, names })
    }
}
