use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SystemDefinition, error};
use crate::Diagnostic;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPersistence {
    Stateless,
    External,
    #[default]
    Private,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub config: Option<BTreeMap<String, Value>>,
    pub configuration: Option<BTreeMap<String, String>>,
    pub persistence: Option<ProviderPersistence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescription {
    pub id: String,
    pub title: String,
    pub description: String,
    pub contracts: Vec<String>,
    pub config: BTreeMap<String, Value>,
    pub configuration: BTreeMap<String, String>,
    pub persistence: ProviderPersistence,
}

fn named(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

impl ProviderDescription {
    pub fn new(
        id: String,
        contracts: Vec<String>,
        metadata: ProviderMetadata,
    ) -> Result<Self, Diagnostic> {
        let description = Self {
            title: metadata.title.unwrap_or_else(|| id.clone()),
            id,
            description: metadata.description.unwrap_or_default(),
            contracts,
            config: metadata.config.unwrap_or_default(),
            configuration: metadata.configuration.unwrap_or_default(),
            persistence: metadata.persistence.unwrap_or_default(),
        };
        description.validate()?;
        Ok(description)
    }

    pub fn validate(&self) -> Result<(), Diagnostic> {
        if !named(&self.id)
            || !named(&self.title)
            || self.contracts.is_empty()
            || !self.contracts.iter().all(|id| named(id))
            || self.contracts.iter().collect::<BTreeSet<_>>().len() != self.contracts.len()
        {
            return Err(error(
                "invalid_provider",
                "Provider identity, title and unique contract identities must be nonempty trimmed strings",
            ));
        }
        crate::encoding::validate_value(
            &serde_json::to_value(&self.config).expect("provider config"),
        )
    }
}

pub fn describe_provider(
    id: String,
    contracts: Vec<String>,
    metadata: Value,
) -> Result<ProviderDescription, Diagnostic> {
    if !metadata.is_object()
        || ["title", "description"]
            .iter()
            .any(|key| metadata.get(key).is_some_and(Value::is_null))
    {
        return Err(error(
            "invalid_provider",
            "Provider metadata must be an object with string title and description",
        ));
    }
    let metadata = serde_json::from_value(metadata)
        .map_err(|cause| error("invalid_provider", cause.to_string()))?;
    ProviderDescription::new(id, contracts, metadata)
}

#[derive(Clone, Debug, Default)]
pub struct ProviderCatalog {
    providers: Vec<ProviderDescription>,
}

impl ProviderCatalog {
    pub fn new(providers: Vec<ProviderDescription>) -> Result<Self, Diagnostic> {
        let mut catalog = Self::default();
        for provider in providers {
            catalog.register(provider)?;
        }
        Ok(catalog)
    }

    pub fn register(&mut self, provider: ProviderDescription) -> Result<(), Diagnostic> {
        provider.validate()?;
        if self.providers.iter().any(|entry| entry.id == provider.id) {
            return Err(error(
                "duplicate_provider",
                format!("Provider {} is already registered", provider.id),
            ));
        }
        self.providers.push(provider);
        Ok(())
    }

    pub fn describe(&self, id: &str) -> Result<&ProviderDescription, Diagnostic> {
        self.providers
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| {
                error(
                    "missing_provider",
                    format!("Provider {id} is not registered"),
                )
            })
    }

    pub fn inspect(&self, contract: Option<&str>) -> Vec<&ProviderDescription> {
        self.providers
            .iter()
            .filter(|entry| {
                contract.is_none_or(|contract| entry.contracts.iter().any(|id| id == contract))
            })
            .collect()
    }

    pub fn check(&self, id: &str, contract: &str) -> Result<(), Diagnostic> {
        if !self.describe(id)?.contracts.iter().any(|id| id == contract) {
            return Err(error(
                "invalid_provider",
                format!("Provider {id} does not implement {contract}"),
            ));
        }
        Ok(())
    }

    pub fn validate_bindings(&self, definition: &SystemDefinition) -> Result<(), Diagnostic> {
        super::validate(definition)?;
        for component in &definition.components {
            for block in &component.blocks {
                let provider = block.provider.as_ref().ok_or_else(|| {
                    error("unbound_block", format!("{}.{}", component.id, block.id))
                })?;
                self.check(provider, &block.contract)?;
            }
        }
        Ok(())
    }
}
