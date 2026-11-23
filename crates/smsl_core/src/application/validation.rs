use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::Map;

use super::{ApplicationDocument, ApplicationInput, ApplicationSource, error, profiles};
use crate::{
    Diagnostic,
    architecture::{Contract, SystemDefinition},
    language::Machine,
    model::DataType,
};

pub(super) fn name(value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.trim().is_empty() {
        return Err(error(format!("{label} must be a nonempty string")));
    }
    Ok(())
}

pub(super) fn document(document: &ApplicationDocument) -> Result<(), Diagnostic> {
    if document.application_version != 1 {
        return Err(error("Unsupported application_version"));
    }
    name(&document.id, "Application id")?;
    for (id, value) in &document.models {
        name(id, "Model id")?;
        source(value)?;
    }
    source(&document.system)?;
    if document.instances.is_empty() {
        return Err(error("instances must be a nonempty array"));
    }
    let mut instances = BTreeSet::new();
    for instance in &document.instances {
        name(&instance.id, "Instance id")?;
        name(&instance.model, "Instance model")?;
        if !instances.insert(instance.id.as_str()) {
            return Err(error(format!("Duplicate instance {}", instance.id)));
        }
        if !document.models.contains_key(&instance.model) {
            return Err(error(format!("Unknown model {}", instance.model)));
        }
        for (key, value) in [
            ("scope", &instance.context.scope),
            ("subject", &instance.context.subject),
            ("clock", &instance.context.clock),
        ] {
            name(value, &format!("Context {key}"))?;
        }
    }
    for (id, binding) in &document.bindings {
        name(id, "Block address")?;
        if !instances.contains(binding.instance.as_str()) {
            return Err(
                error(format!("Unknown instance {} for {id}", binding.instance))
                    .at_path(format!("/bindings/{}/instance", crate::diagnostic::key(id))),
            );
        }
        for (port, input) in &binding.inputs {
            name(port, "Input port")?;
            match input {
                ApplicationInput::Host { host: false } => {
                    return Err(error(format!("Host input {id}.{port} must be true")));
                }
                ApplicationInput::Profile { profile, .. } => name(profile, "Profile id")?,
                _ => {}
            }
        }
        if let Some(lifecycle) = &binding.lifecycle {
            name(&lifecycle.event, "Lifecycle event")?;
            if !instances.contains(lifecycle.instance.as_str()) {
                return Err(error(format!(
                    "Unknown lifecycle instance {}",
                    lifecycle.instance
                )));
            }
            if lifecycle.instance == binding.instance {
                return Err(error(
                    "Lifecycle events require a separate controller instance",
                ));
            }
        }
        for (port, validator) in binding.outputs.iter().flatten() {
            name(port, "Output port")?;
            name(validator, &format!("Output validator {id}.{port}"))?;
        }
        for (key, reference) in binding.configuration.iter().flatten() {
            name(key, "Configuration key")?;
            if !document.models.contains_key(&reference.model) {
                return Err(error(format!(
                    "Unknown configuration model {}",
                    reference.model
                )));
            }
        }
    }
    crate::encoding::validate_value(&serde_json::to_value(document).expect("application document"))
}

fn source(source: &ApplicationSource) -> Result<(), Diagnostic> {
    if let ApplicationSource::Reference { r#ref } = source {
        name(r#ref, "Source reference")?;
    }
    Ok(())
}

pub(super) fn bindings(
    document: &ApplicationDocument,
    models: &BTreeMap<String, Arc<Machine>>,
    system: &SystemDefinition,
    contracts: &[Contract],
    profiles: &BTreeMap<String, String>,
) -> Result<(), Diagnostic> {
    let contracts: BTreeMap<_, _> = contracts
        .iter()
        .map(|contract| (contract.id.as_str(), contract))
        .collect();
    let targets: BTreeSet<_> = system
        .connections
        .iter()
        .map(|connection| connection.to.as_str())
        .collect();
    for id in document.bindings.keys() {
        if system.block(id).is_none() {
            return Err(error(format!("Unknown block {id}")));
        }
    }
    let empty = Map::new();
    for component in &system.components {
        for block in &component.blocks {
            let id = format!("{}.{}", component.id, block.id);
            let binding = document
                .bindings
                .get(&id)
                .ok_or_else(|| error(format!("Missing binding for {id}")))?;
            let instance = document
                .instances
                .iter()
                .find(|instance| instance.id == binding.instance)
                .expect("validated instance");
            let contract = contracts[block.contract.as_str()];
            if let Some(lifecycle) = &binding.lifecycle {
                let target = document
                    .instances
                    .iter()
                    .find(|instance| instance.id == lifecycle.instance)
                    .expect("validated lifecycle instance");
                let event = models[&target.model]
                    .document()
                    .events
                    .iter()
                    .find(|event| event.name == lifecycle.event)
                    .ok_or_else(|| error(format!("Unknown lifecycle event {}", lifecycle.event)))?;
                let fields = BTreeMap::from([
                    ("kind", DataType::String),
                    ("block", DataType::String),
                    ("request_id", DataType::String),
                    ("basis_revision", DataType::String),
                    ("basis_tick", DataType::UInt),
                ]);
                if event.fields.len() != fields.len()
                    || event.fields.iter().any(|field| {
                        field.nullable || fields.get(field.name.as_str()) != Some(&field.data_type)
                    })
                {
                    return Err(error(
                        "Lifecycle events require nonnullable kind, block, request_id, basis_revision strings and basis_tick uint",
                    ));
                }
            }
            for (port, input) in &binding.inputs {
                let declaration = contract
                    .inputs
                    .get(port)
                    .ok_or_else(|| error(format!("Unknown input {id}.{port}")))?;
                if targets.contains(format!("{id}.{port}").as_str()) {
                    return Err(error(format!(
                        "Connected input {id}.{port} cannot have a second binding"
                    )));
                }
                if let ApplicationInput::Profile { profile, config } = input {
                    let schema = profiles
                        .get(profile)
                        .ok_or_else(|| error(format!("Unknown profile {profile}")))?;
                    if schema != &declaration.schema {
                        return Err(error(format!("Profile schema mismatch for {id}.{port}")));
                    }
                    if profiles::standard(profile) {
                        profiles::validate_profile(
                            profile,
                            &models[&instance.model],
                            config.as_ref().unwrap_or(&empty),
                        )?;
                    }
                }
            }
            for (port, declaration) in &contract.inputs {
                if declaration.required
                    && !binding.inputs.contains_key(port)
                    && !targets.contains(format!("{id}.{port}").as_str())
                {
                    return Err(error(format!(
                        "Required input {id}.{port} needs a binding or connection"
                    )));
                }
            }
            for key in binding.configuration.iter().flatten().map(|(key, _)| key) {
                if block.config.get(key).is_some() {
                    return Err(error(format!(
                        "Configuration {id}.{key} already has a literal value"
                    )));
                }
            }
            for port in binding.outputs.iter().flatten().map(|(port, _)| port) {
                if !contract.outputs.contains_key(port) {
                    return Err(error(format!("Unknown output {id}.{port}")));
                }
            }
        }
    }
    Ok(())
}
