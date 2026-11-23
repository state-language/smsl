use super::parser::{Object, Parser, Result, child};
use crate::source::lexer::Kind;
use serde_json::{Value, json};

impl Parser {
    pub fn application(&mut self) -> Result<Value> {
        let start = self.tokens[self.index - 1].location.offset;
        let id = self.name("/id")?;
        self.expect(Kind::OpenBranch)?;
        let mut root = Object::from_iter([
            ("application_version".into(), 1.into()),
            ("id".into(), id.into()),
        ]);
        let (mut models, mut instances, mut bindings) = (Object::new(), Vec::new(), Object::new());
        while !self.consume(Kind::CloseBranch) {
            let start = self.start();
            if self.word("version") {
                self.unique("/application_version")?;
                root.insert(
                    "application_version".into(),
                    self.number("/application_version")?.into(),
                );
                self.end()?;
            } else if self.word("model") {
                let name = self.name("")?;
                let path = child("/models", &name);
                self.unique(&path)?;
                models.insert(name, self.source(&path)?);
                self.end()?;
                self.span(&path, start);
            } else if self.word("system") {
                self.unique("/system")?;
                root.insert("system".into(), self.source("/system")?);
                self.end()?;
                self.span("/system", start);
            } else if self.word("instance") {
                let path = format!("/instances/{}", instances.len());
                let id = self.name(&child(&path, "id"))?;
                self.unique(&child("@instances", &id))?;
                self.expect(Kind::Colon)?;
                let model = self.name(&child(&path, "model"))?;
                self.expect(Kind::OpenBranch)?;
                let mut context = Object::new();
                while !self.consume(Kind::CloseBranch) {
                    let field = self.name("")?;
                    if !matches!(field.as_str(), "scope" | "subject" | "clock") {
                        return Err(self.error("Expected scope, subject or clock"));
                    }
                    self.field_string(&mut context, &field, &child(&path, "context"))?;
                    self.end()?;
                }
                for field in ["scope", "subject", "clock"] {
                    if !context.contains_key(field) {
                        return Err(self.error(format!("Instance context {field} is required")));
                    }
                }
                instances.push(json!({"id":id,"model":model,"context":context}));
                self.span(&path, start);
            } else if self.word("bind") {
                let address = self.name("")?;
                let path = child("/bindings", &address);
                self.unique(&path)?;
                bindings.insert(address, self.binding(&path)?);
                self.span(&path, start);
            } else {
                return Err(self.error("Expected version, model, system, instance or bind"));
            }
        }
        if !root.contains_key("system") {
            return Err(self
                .error("Application system source is required")
                .at_path("/system"));
        }
        root.insert("models".into(), models.into());
        root.insert("instances".into(), instances.into());
        root.insert("bindings".into(), bindings.into());
        self.span("", start);
        Ok(root.into())
    }
    fn source(&mut self, path: &str) -> Result<Value> {
        if self.word("from") {
            Ok(json!({"ref":self.string(&child(path, "ref"))?}))
        } else if self.word("embedded") {
            let format = self.name(&child(path, "format"))?;
            let text = self.string(&child(path, "text"))?;
            Ok(json!({"format":format,"text":text}))
        } else {
            Err(self.error("Expected from or embedded"))
        }
    }
    fn binding(&mut self, path: &str) -> Result<Value> {
        self.expect_word("to")?;
        let instance = self.name(&child(path, "instance"))?;
        self.expect(Kind::OpenBranch)?;
        let mut root = Object::from_iter([("instance".into(), instance.into())]);
        let mut inputs = Object::new();
        while !self.consume(Kind::CloseBranch) {
            if self.word("input") {
                self.binding_input(&mut inputs, &child(path, "inputs"))?;
            } else if self.word("output") {
                self.binding_output(&mut root, path)?;
            } else if self.word("outputs") {
                self.unique(&format!("@{path}/outputs"))?;
                root.entry("outputs").or_insert(json!({}));
                self.expect(Kind::OpenBranch)?;
                while !self.consume(Kind::CloseBranch) {
                    self.expect_word("output")?;
                    self.binding_output(&mut root, path)?;
                }
            } else if self.word("configuration") {
                if self.consume(Kind::OpenBranch) {
                    self.unique(&format!("@{path}/configuration"))?;
                    root.entry("configuration").or_insert(json!({}));
                    while !self.consume(Kind::CloseBranch) {
                        self.configuration(&mut root, path)?;
                    }
                } else {
                    self.configuration(&mut root, path)?;
                }
            } else if self.word("lifecycle") {
                let field = child(path, "lifecycle");
                self.unique(&field)?;
                self.expect_word("to")?;
                let instance = self.name(&child(&field, "instance"))?;
                self.expect_word("event")?;
                let event = self.name(&child(&field, "event"))?;
                self.end()?;
                root.insert(
                    "lifecycle".into(),
                    json!({"instance":instance,"event":event}),
                );
            } else {
                return Err(
                    self.error("Expected input, output, outputs, configuration or lifecycle")
                );
            }
        }
        root.insert("inputs".into(), inputs.into());
        Ok(root.into())
    }
    fn binding_input(&mut self, inputs: &mut Object, path: &str) -> Result<()> {
        let start = self.tokens[self.index - 1].location.offset;
        let name = self.name("")?;
        let path = child(path, &name);
        self.unique(&path)?;
        let input = if self.consume(Kind::Assign) {
            json!({"value":self.value(&child(&path,"value"))?})
        } else {
            self.expect_word("from")?;
            if self.word("host") {
                self.span(
                    &child(&path, "host"),
                    self.tokens[self.index - 1].location.offset,
                );
                json!({"host":true})
            } else {
                self.expect_word("profile")?;
                let mut value = Object::from_iter([(
                    "profile".into(),
                    self.name(&child(&path, "profile"))?.into(),
                )]);
                if self.word("config") {
                    value.insert("config".into(), self.value(&child(&path, "config"))?);
                }
                value.into()
            }
        };
        self.end()?;
        self.span(&path, start);
        inputs.insert(name, input);
        Ok(())
    }
    fn binding_output(&mut self, root: &mut Object, path: &str) -> Result<()> {
        let name = self.name("")?;
        let field = child(&child(path, "outputs"), &name);
        self.unique(&field)?;
        self.expect_word("as")?;
        let profile = self.name(&field)?;
        self.end()?;
        root.entry("outputs")
            .or_insert(json!({}))
            .as_object_mut()
            .expect("output map")
            .insert(name, profile.into());
        Ok(())
    }
    fn configuration(&mut self, root: &mut Object, path: &str) -> Result<()> {
        let name = self.name("")?;
        let field = child(&child(path, "configuration"), &name);
        self.unique(&field)?;
        self.expect_word("from")?;
        self.expect_word("model")?;
        let model = self.name(&child(&field, "model"))?;
        self.end()?;
        root.entry("configuration")
            .or_insert(json!({}))
            .as_object_mut()
            .expect("configuration map")
            .insert(name, json!({"model":model}));
        Ok(())
    }
}
