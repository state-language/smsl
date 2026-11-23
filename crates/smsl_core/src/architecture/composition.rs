use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Contract;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemDefinition {
    pub system_version: u32,
    pub id: String,
    #[serde(default = "general")]
    pub profile: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub imports: Vec<String>,
    #[serde(default)]
    pub contracts: Vec<Contract>,
    pub components: Vec<Component>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub id: String,
    pub contract: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default = "object")]
    pub config: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub delay: u8,
}

impl SystemDefinition {
    pub fn block(&self, id: &str) -> Option<&Block> {
        let (component, block) = id.split_once('.')?;
        self.components
            .iter()
            .find(|entry| entry.id == component)?
            .blocks
            .iter()
            .find(|entry| entry.id == block)
    }
}

fn general() -> String {
    "general".into()
}
fn object() -> Value {
    Value::Object(Default::default())
}
