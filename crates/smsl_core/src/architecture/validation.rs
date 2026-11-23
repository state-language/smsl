use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use super::{Contract, Port, SystemDefinition, catalog, error};
use crate::Diagnostic;

#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub id: String,
    pub profile: String,
    pub components: usize,
    pub blocks: usize,
    pub host_inputs: Vec<String>,
    pub unbound_blocks: Vec<String>,
    pub contracts: Vec<Contract>,
}

pub fn validate(definition: &SystemDefinition) -> Result<Inspection, Diagnostic> {
    if definition.system_version != 1 {
        return Err(error("system_version", "Expected system_version 1"));
    }
    identifier(&definition.id)?;
    identifier(&definition.profile)?;
    if !definition.imports.is_empty() {
        return Err(error(
            "unresolved_import",
            "Link system imports before validation",
        ));
    }
    for (key, value) in &definition.aliases {
        if key.trim().is_empty() || value.trim().is_empty() {
            return Err(error(
                "invalid_alias",
                "Alias keys and labels must not be empty",
            ));
        }
    }
    let contracts = contracts(definition)?;
    let mut components = BTreeSet::new();
    let mut blocks = BTreeMap::new();
    let mut unbound_blocks = Vec::new();
    for component in &definition.components {
        identifier(&component.id)?;
        if component.role.trim().is_empty()
            || component
                .label
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(error(
                "invalid_component",
                "Component roles and labels must not be empty",
            ));
        }
        if !components.insert(&component.id) {
            return Err(error("duplicate_component", &component.id));
        }
        for block in &component.blocks {
            identifier(&block.id)?;
            if !block.config.is_object() {
                return Err(error(
                    "invalid_config",
                    "Provider configuration must be an object",
                ));
            }
            crate::encoding::validate_value(&block.config)?;
            let id = format!("{}.{}", component.id, block.id);
            let contract = contracts
                .get(&block.contract)
                .ok_or_else(|| error("unknown_contract", &block.contract))?;
            if blocks.insert(id.clone(), contract).is_some() {
                return Err(error("duplicate_block", id));
            }
            match &block.provider {
                Some(provider) if provider.trim().is_empty() => {
                    return Err(error(
                        "invalid_provider",
                        "Provider identity must not be empty",
                    ));
                }
                None => unbound_blocks.push(id),
                _ => {}
            }
        }
    }
    let mut incoming = BTreeSet::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut degrees: BTreeMap<String, usize> = blocks.keys().map(|key| (key.clone(), 0)).collect();
    for connection in &definition.connections {
        if connection.delay > 1 {
            return Err(error("invalid_delay", "Connection delay must be 0 or 1"));
        }
        let (from_block, from) = endpoint(&connection.from)?;
        let (to_block, to) = endpoint(&connection.to)?;
        let source = blocks
            .get(from_block)
            .and_then(|contract| contract.outputs.get(from))
            .ok_or_else(|| error("unknown_output", &connection.from))?;
        let target = blocks
            .get(to_block)
            .and_then(|contract| contract.inputs.get(to))
            .ok_or_else(|| error("unknown_input", &connection.to))?;
        if source.schema != target.schema
            || source.unit != target.unit
            || source.frame != target.frame
        {
            return Err(error(
                "incompatible_ports",
                format!(
                    "{} -> {} requires matching schema, unit and frame",
                    connection.from, connection.to
                ),
            ));
        }
        if connection.delay == 1 && target.freshness != super::Freshness::Any {
            return Err(error(
                "invalid_feedback",
                format!(
                    "{} requires freshness any for delayed feedback",
                    connection.to
                ),
            ));
        }
        if !incoming.insert(connection.to.clone()) {
            return Err(error("duplicate_connection", &connection.to));
        }
        if connection.delay == 0 {
            edges
                .entry(from_block.into())
                .or_default()
                .push(to_block.into());
            *degrees.get_mut(to_block).expect("validated block") += 1;
        }
    }
    let mut ready: VecDeque<_> = degrees
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for target in edges.get(&id).into_iter().flatten() {
            let degree = degrees.get_mut(target).expect("validated edge");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(target.clone());
            }
        }
    }
    if visited != blocks.len() {
        return Err(error(
            "instantaneous_cycle",
            "Feedback cycles require an explicit delayed connection",
        ));
    }
    let host_inputs = blocks
        .iter()
        .flat_map(|(id, contract)| {
            contract
                .inputs
                .iter()
                .filter(|(_, port)| port.required)
                .map(move |(name, _)| format!("{id}.{name}"))
        })
        .filter(|endpoint| !incoming.contains(endpoint))
        .collect();
    Ok(Inspection {
        id: definition.id.clone(),
        profile: definition.profile.clone(),
        components: components.len(),
        blocks: blocks.len(),
        host_inputs,
        unbound_blocks,
        contracts: contracts.into_values().collect(),
    })
}

pub(crate) fn contracts(
    definition: &SystemDefinition,
) -> Result<BTreeMap<String, Contract>, Diagnostic> {
    let mut contracts: BTreeMap<_, _> = catalog()
        .into_iter()
        .map(|contract| (contract.id.clone(), contract))
        .collect();
    for contract in &definition.contracts {
        for part in contract.id.split('.') {
            identifier(part)?;
        }
        for (name, port) in contract.inputs.iter().chain(&contract.outputs) {
            identifier(name)?;
            validate_port(port)?;
        }
        if contracts
            .insert(contract.id.clone(), contract.clone())
            .is_some()
        {
            return Err(error("duplicate_contract", &contract.id));
        }
    }
    Ok(contracts)
}

pub(crate) fn endpoint(value: &str) -> Result<(&str, &str), Diagnostic> {
    let (block, port) = value
        .rsplit_once('.')
        .ok_or_else(|| error("invalid_endpoint", value))?;
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return Err(error("invalid_endpoint", value));
    }
    for part in parts {
        identifier(part)?;
    }
    Ok((block, port))
}

fn validate_port(port: &Port) -> Result<(), Diagnostic> {
    if port.schema.trim().is_empty()
        || port
            .unit
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || port
            .frame
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(error(
            "invalid_port",
            "Port schema, unit and frame must not be empty",
        ));
    }
    Ok(())
}

fn identifier(value: &str) -> Result<(), Diagnostic> {
    let mut chars = value.chars();
    if !chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(error("invalid_identifier", value));
    }
    Ok(())
}
