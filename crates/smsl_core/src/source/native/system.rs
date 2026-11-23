use super::parser::{Object, Parser, Result, child};
use crate::source::lexer::Kind;
use serde_json::{Value, json};

impl Parser {
    pub fn system(&mut self) -> Result<Value> {
        let start = self.tokens[self.index - 1].location.offset;
        let id = self.name("/id")?;
        self.expect(Kind::OpenBranch)?;
        let mut root = Object::from_iter([
            ("system_version".into(), 1.into()),
            ("id".into(), id.into()),
            ("profile".into(), "general".into()),
        ]);
        let mut aliases = Object::new();
        let (mut imports, mut contracts, mut components, mut connections) = (
            Vec::<Value>::new(),
            Vec::<Value>::new(),
            Vec::<Value>::new(),
            Vec::<Value>::new(),
        );
        while !self.consume(Kind::CloseBranch) {
            let start = self.start();
            if self.word("version") {
                self.unique("/system_version")?;
                root.insert(
                    "system_version".into(),
                    self.number("/system_version")?.into(),
                );
                self.end()?;
            } else if self.word("profile") {
                self.field_name(&mut root, "profile", "")?;
                self.end()?;
            } else if self.word("alias") {
                let key = self.name("")?;
                let path = child("/aliases", &key);
                self.unique(&path)?;
                self.expect(Kind::Assign)?;
                aliases.insert(key, self.string(&path)?.into());
                self.end()?;
            } else if self.word("import") {
                imports.push(self.string(&format!("/imports/{}", imports.len()))?.into());
                self.end()?;
            } else if self.word("contract") {
                let path = format!("/contracts/{}", contracts.len());
                contracts.push(self.contract(&path)?);
                self.span(&path, start);
            } else if self.word("component") {
                let path = format!("/components/{}", components.len());
                components.push(self.component(&path)?);
                self.span(&path, start);
            } else if self.word("connect") {
                let path = format!("/connections/{}", connections.len());
                let from = self.name(&child(&path, "from"))?;
                self.expect(Kind::Arrow)?;
                let to = self.name(&child(&path, "to"))?;
                let delay = if self.word("delay") {
                    self.number(&child(&path, "delay"))?
                } else {
                    0
                };
                self.end()?;
                self.span(&path, start);
                connections.push(json!({"from":from, "to":to, "delay":delay}));
            } else {
                return Err(self.error(
                    "Expected version, profile, alias, import, contract, component or connect",
                ));
            }
        }
        root.insert("aliases".into(), aliases.into());
        root.insert("imports".into(), imports.into());
        root.insert("contracts".into(), contracts.into());
        root.insert("components".into(), components.into());
        root.insert("connections".into(), connections.into());
        self.span("", start);
        Ok(root.into())
    }
    fn contract(&mut self, path: &str) -> Result<Value> {
        let id = self.name(&child(path, "id"))?;
        self.unique(&child("@contracts", &id))?;
        self.expect(Kind::OpenBranch)?;
        let mut root = Object::from_iter([("id".into(), id.into())]);
        let (mut inputs, mut outputs) = (Object::new(), Object::new());
        while !self.consume(Kind::CloseBranch) {
            if self.word("input") {
                self.port(&mut inputs, &child(path, "inputs"))?;
            } else if self.word("output") {
                self.port(&mut outputs, &child(path, "outputs"))?;
            } else if self.word("effect") {
                self.field_choice(
                    &mut root,
                    "effect",
                    path,
                    &["pure", "evolution", "execution"],
                )?;
                self.end()?;
            } else {
                return Err(self.error("Expected input, output or effect"));
            }
        }
        if !root.contains_key("effect") {
            return Err(self
                .error("Contract effect is required")
                .at_path(child(path, "effect")));
        }
        root.insert("inputs".into(), inputs.into());
        root.insert("outputs".into(), outputs.into());
        Ok(root.into())
    }
    fn port(&mut self, ports: &mut Object, path: &str) -> Result<()> {
        let start = self.tokens[self.index - 1].location.offset;
        let name = self.name("")?;
        let path = child(path, &name);
        self.unique(&path)?;
        self.expect(Kind::Colon)?;
        let schema = self.name(&child(&path, "schema"))?;
        let mut port = Object::from_iter([
            ("schema".into(), schema.into()),
            ("required".into(), true.into()),
            ("freshness".into(), "current".into()),
        ]);
        while !self.consume(Kind::Semicolon) {
            if self.word("optional") {
                let field = child(&path, "required");
                self.unique(&field)?;
                self.span(&field, self.tokens[self.index - 1].location.offset);
                port.insert("required".into(), false.into());
            } else if self.word("unit") {
                self.field_string(&mut port, "unit", &path)?;
            } else if self.word("frame") {
                self.field_string(&mut port, "frame", &path)?;
            } else if self.word("freshness") {
                self.field_choice(&mut port, "freshness", &path, &["current", "any"])?;
            } else {
                return Err(self.error("Expected optional, unit, frame, freshness or ';'"));
            }
        }
        self.span(&path, start);
        ports.insert(name, port.into());
        Ok(())
    }
    fn component(&mut self, path: &str) -> Result<Value> {
        let id = self.name(&child(path, "id"))?;
        self.unique(&child("@components", &id))?;
        self.expect(Kind::Colon)?;
        let role = self.name(&child(path, "role"))?;
        self.expect(Kind::OpenBranch)?;
        let mut root = Object::from_iter([("id".into(), id.into()), ("role".into(), role.into())]);
        let mut blocks = Vec::<Value>::new();
        while !self.consume(Kind::CloseBranch) {
            let start = self.start();
            if self.word("label") {
                self.field_string(&mut root, "label", path)?;
                self.end()?;
            } else if self.word("block") {
                let block_path = format!("{path}/blocks/{}", blocks.len());
                let id = self.name(&child(&block_path, "id"))?;
                self.unique(&child(&format!("@{path}/blocks"), &id))?;
                self.expect(Kind::Colon)?;
                let contract = self.name(&child(&block_path, "contract"))?;
                self.expect(Kind::OpenBranch)?;
                let mut block = Object::from_iter([
                    ("id".into(), id.into()),
                    ("contract".into(), contract.into()),
                    ("config".into(), json!({})),
                ]);
                while !self.consume(Kind::CloseBranch) {
                    if self.word("provider") {
                        self.field_string(&mut block, "provider", &block_path)?;
                    } else if self.word("config") {
                        let field = child(&block_path, "config");
                        self.unique(&field)?;
                        block.insert("config".into(), self.value(&field)?);
                    } else {
                        return Err(self.error("Expected provider or config"));
                    }
                    self.end()?;
                }
                self.span(&block_path, start);
                blocks.push(block.into());
            } else {
                return Err(self.error("Expected label or block"));
            }
        }
        root.insert("blocks".into(), blocks.into());
        Ok(root.into())
    }
}
