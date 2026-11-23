use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::{
    architecture::SystemDefinition,
    encoding::unique_map,
    language::{model::Document, semantics::Context},
    source::SourceFormat,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationDocument {
    pub application_version: u64,
    pub id: String,
    #[serde(deserialize_with = "unique_map")]
    pub models: BTreeMap<String, ApplicationSource>,
    pub instances: Vec<ApplicationInstance>,
    pub system: ApplicationSource,
    #[serde(deserialize_with = "unique_map")]
    pub bindings: BTreeMap<String, ApplicationBlock>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ApplicationSource {
    Reference { r#ref: String },
    Embedded { text: String, format: SourceFormat },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationInstance {
    pub id: String,
    pub model: String,
    pub context: Context,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationBlock {
    pub instance: String,
    #[serde(deserialize_with = "unique_map")]
    pub inputs: BTreeMap<String, ApplicationInput>,
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub outputs: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub configuration: Option<BTreeMap<String, ModelReference>>,
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub lifecycle: Option<ApplicationLifecycle>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationLifecycle {
    pub instance: String,
    pub event: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ApplicationInput {
    Value {
        #[serde(deserialize_with = "Value::deserialize")]
        value: Value,
    },
    Host {
        host: bool,
    },
    Profile {
        profile: String,
        #[serde(
            default,
            deserialize_with = "optional",
            skip_serializing_if = "Option::is_none"
        )]
        config: Option<Map<String, Value>>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelReference {
    pub model: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationProfileDeclaration {
    pub id: String,
    pub schema: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApplicationInspection {
    pub id: String,
    pub models: Vec<String>,
    pub instances: Vec<ApplicationInstance>,
    pub blocks: Vec<ApplicationBlockInspection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApplicationBlockInspection {
    pub id: String,
    pub role: String,
    pub contract: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub binding: ApplicationBlock,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedApplicationModel {
    pub definition: Document,
    pub source: String,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApplicationProjection {
    pub port: String,
    pub profile: String,
    pub config: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApplicationBindingPlan {
    pub instance: String,
    pub model: String,
    pub projections: Vec<ApplicationProjection>,
    pub outputs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoadedApplication {
    pub document: ApplicationDocument,
    pub models: BTreeMap<String, ResolvedApplicationModel>,
    pub system: SystemDefinition,
    pub inspection: ApplicationInspection,
    pub binding_plans: BTreeMap<String, ApplicationBindingPlan>,
}

fn optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
