use std::{collections::BTreeMap, sync::Arc};

use serde_json::Value;

use super::{
    ApplicationBindingPlan, ApplicationBlockInspection, ApplicationDocument, ApplicationInput,
    ApplicationInspection, ApplicationProfileDeclaration, ApplicationProjection, ApplicationSource,
    LoadedApplication, ResolvedApplicationModel, error, profiles, validation,
};
use crate::{
    Diagnostic,
    architecture::{self, Contract, SystemDefinition},
    language::{self, Machine},
    source::{SourceFormat, SourceText},
};

#[derive(Debug)]
pub struct ApplicationDefinition {
    document: ApplicationDocument,
    models: BTreeMap<String, Arc<Machine>>,
    system: SystemDefinition,
    contracts: BTreeMap<String, Contract>,
}

impl ApplicationDefinition {
    pub fn new(
        document: ApplicationDocument,
        sources: &BTreeMap<String, SourceText>,
        profiles: &[ApplicationProfileDeclaration],
    ) -> Result<Self, Diagnostic> {
        validation::document(&document)?;
        let profiles = profiles::declarations(profiles)?;
        let mut models = BTreeMap::new();
        for (id, source) in &document.models {
            let machine = match source {
                ApplicationSource::Reference { r#ref } => {
                    language::modules::link_sources(r#ref, sources)?
                }
                ApplicationSource::Embedded {
                    text,
                    format: SourceFormat::Smsl,
                } => language::load(text)?,
                ApplicationSource::Embedded { text, format } => {
                    language::codec::from_encoding(text, format.encoding().expect("data encoding"))?
                }
            };
            models.insert(id.clone(), Arc::new(machine));
        }
        let mut system = match &document.system {
            ApplicationSource::Reference { r#ref } => architecture::link_sources(r#ref, sources)?,
            ApplicationSource::Embedded { text, format } => architecture::decode(text, *format)?,
        };
        let inspection = architecture::validate(&system)?;
        validation::bindings(
            &document,
            &models,
            &system,
            &inspection.contracts,
            &profiles,
        )?;
        for component in &mut system.components {
            for block in &mut component.blocks {
                let id = format!("{}.{}", component.id, block.id);
                let config = block
                    .config
                    .as_object_mut()
                    .expect("validated provider configuration");
                for (key, reference) in document.bindings[&id].configuration.iter().flatten() {
                    config.insert(
                        key.clone(),
                        Value::String(models[&reference.model].canonical_source().to_string()),
                    );
                }
                crate::encoding::validate_value(&block.config)?;
            }
        }
        Ok(Self {
            document,
            models,
            system,
            contracts: inspection
                .contracts
                .into_iter()
                .map(|contract| (contract.id.clone(), contract))
                .collect(),
        })
    }

    pub fn document(&self) -> &ApplicationDocument {
        &self.document
    }

    pub fn model(&self, id: &str) -> Result<&Arc<Machine>, Diagnostic> {
        self.models
            .get(id)
            .ok_or_else(|| error(format!("Unknown model {id}")))
    }

    pub fn system_definition(&self) -> &SystemDefinition {
        &self.system
    }

    pub fn block_contract(&self, block: &str) -> Result<&Contract, Diagnostic> {
        let block = self
            .system
            .block(block)
            .ok_or_else(|| error(format!("Unknown block {block}")))?;
        Ok(&self.contracts[&block.contract])
    }

    pub fn binding_plan(&self, block: &str) -> Result<ApplicationBindingPlan, Diagnostic> {
        let binding = self
            .document
            .bindings
            .get(block)
            .ok_or_else(|| error(format!("Unknown block {block}")))?;
        let instance = self
            .document
            .instances
            .iter()
            .find(|instance| instance.id == binding.instance)
            .expect("validated binding");
        Ok(ApplicationBindingPlan {
            instance: instance.id.clone(),
            model: instance.model.clone(),
            projections: binding
                .inputs
                .iter()
                .filter_map(|(port, input)| match input {
                    ApplicationInput::Profile { profile, config }
                        if !profiles::standard(profile) =>
                    {
                        Some(ApplicationProjection {
                            port: port.clone(),
                            profile: profile.clone(),
                            config: config.clone().unwrap_or_default(),
                        })
                    }
                    _ => None,
                })
                .collect(),
            outputs: binding.outputs.clone().unwrap_or_default(),
        })
    }

    pub fn inspect(&self) -> ApplicationInspection {
        ApplicationInspection {
            id: self.document.id.clone(),
            models: self.models.keys().cloned().collect(),
            instances: self.document.instances.clone(),
            blocks: self
                .system
                .components
                .iter()
                .flat_map(|component| {
                    component.blocks.iter().map(|block| {
                        let id = format!("{}.{}", component.id, block.id);
                        ApplicationBlockInspection {
                            binding: self.document.bindings[&id].clone(),
                            id,
                            role: component.role.clone(),
                            contract: block.contract.clone(),
                            provider: block.provider.clone(),
                        }
                    })
                })
                .collect(),
        }
    }

    pub fn resolved(&self) -> LoadedApplication {
        LoadedApplication {
            document: self.document.clone(),
            models: self
                .models
                .iter()
                .map(|(id, machine)| {
                    (
                        id.clone(),
                        ResolvedApplicationModel {
                            definition: machine.document().clone(),
                            source: machine.canonical_source().to_string(),
                            revision: machine.revision().into(),
                        },
                    )
                })
                .collect(),
            system: self.system.clone(),
            inspection: self.inspect(),
            binding_plans: self
                .document
                .bindings
                .keys()
                .map(|block| {
                    (
                        block.clone(),
                        self.binding_plan(block).expect("validated block"),
                    )
                })
                .collect(),
        }
    }
}
