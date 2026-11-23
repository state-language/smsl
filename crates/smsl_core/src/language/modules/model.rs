use serde::{Deserialize, Serialize};

use crate::language::model::{
    Branch, Datum, Document, EventDeclaration, EventField, UIntRange, Value,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<Import>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<String>,
    pub events: Vec<EventDeclaration>,
    pub branches: Vec<Branch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<Template>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<Instance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<Domain>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    pub name: String,
    pub values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<UIntRange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub parameters: Vec<EventField>,
    pub branch: Branch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub name: String,
    pub template: String,
    #[serde(deserialize_with = "crate::encoding::unique_map")]
    pub arguments: BTreeMap<String, Datum>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub source: String,
    pub names: Vec<ImportedName>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedName {
    pub name: String,
    pub alias: String,
}

impl From<Document> for Module {
    fn from(document: Document) -> Self {
        Self {
            version: document.version,
            imports: Vec::new(),
            exports: Vec::new(),
            events: document.events,
            branches: document.branches,
            templates: Vec::new(),
            instances: Vec::new(),
            domains: Vec::new(),
        }
    }
}

impl Module {
    pub fn document(&self) -> Document {
        Document {
            version: self.version.clone(),
            events: self.events.clone(),
            branches: self.branches.clone(),
        }
    }
}
