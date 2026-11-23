use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::runtime::{
    RuntimeState, World,
    system::{Basis, Record, Records},
};
use crate::{
    Diagnostic,
    application::{
        ApplicationBindingPlan, ApplicationDefinition, ApplicationInput, ApplicationInstance,
        standard,
    },
};

use super::{check_basis, error, profile};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureOptions {
    pub block: String,
    #[serde(default)]
    pub host_inputs: BTreeMap<String, Value>,
    #[serde(default = "initial_sequence")]
    pub sequence: u64,
    #[serde(default)]
    pub projections: BTreeMap<String, Value>,
    #[serde(default)]
    pub basis: Option<Basis>,
}

fn initial_sequence() -> u64 {
    1
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub instance: String,
    pub basis: Basis,
    pub inputs: Records,
    pub outputs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CapturePlan {
    #[serde(flatten)]
    pub binding: ApplicationBindingPlan,
    pub basis: Basis,
}

pub fn create_world(application: &ApplicationDefinition) -> Result<World, Diagnostic> {
    let mut world = World::new();
    for instance in &application.document().instances {
        world.spawn(
            &instance.id,
            Arc::clone(application.model(&instance.model)?),
            instance.context.clone(),
        )?;
    }
    Ok(world)
}

fn state<'a>(
    application: &ApplicationDefinition,
    world: &'a World,
    instance: &ApplicationInstance,
) -> Result<&'a RuntimeState, Diagnostic> {
    let state = world
        .state(&instance.id)
        .ok_or_else(|| error(format!("World instance {} is missing", instance.id)))?;
    if state.revision() != application.model(&instance.model)?.revision() {
        return Err(error(format!(
            "World instance {} has a different model",
            instance.id
        )));
    }
    Ok(state)
}

pub fn validate_world(
    application: &ApplicationDefinition,
    world: &World,
) -> Result<(), Diagnostic> {
    for instance in &application.document().instances {
        let state = state(application, world, instance)?;
        if state.context() != &instance.context {
            return Err(error(format!(
                "World instance {} has a different context",
                instance.id
            )));
        }
    }
    Ok(())
}

pub fn capture_plan(
    application: &ApplicationDefinition,
    world: &World,
    block: &str,
) -> Result<CapturePlan, Diagnostic> {
    let binding = application.binding_plan(block)?;
    let instance = application
        .document()
        .instances
        .iter()
        .find(|instance| instance.id == binding.instance)
        .expect("validated binding");
    let basis = Basis::from_state(state(application, world, instance)?);
    Ok(CapturePlan { binding, basis })
}

/// Capture after `validate_world` attaches the application. A later World reset
/// changes its basis while retaining the bound model.
pub fn capture(
    application: &ApplicationDefinition,
    world: &World,
    options: &CaptureOptions,
) -> Result<Capture, Diagnostic> {
    let block = &options.block;
    let binding = application
        .document()
        .bindings
        .get(block)
        .ok_or_else(|| error(format!("Unknown block {block}")))?;
    let contract = application.block_contract(block)?;
    let instance = application
        .document()
        .instances
        .iter()
        .find(|instance| instance.id == binding.instance)
        .expect("validated binding");
    let state = state(application, world, instance)?;
    let basis = Basis::from_state(state);
    if let Some(expected) = &options.basis {
        check_basis(world, &instance.id, expected)?;
    }
    for port in options.host_inputs.keys() {
        if !matches!(
            binding.inputs.get(port),
            Some(ApplicationInput::Host { .. })
        ) {
            return Err(error(format!("Input {block}.{port} is not a host input")));
        }
    }
    for port in options.projections.keys() {
        if !matches!(binding.inputs.get(port), Some(ApplicationInput::Profile { profile, .. }) if !standard(profile))
        {
            return Err(error(format!(
                "Input {block}.{port} is not a custom projection"
            )));
        }
    }
    let mut inputs = Records::new();
    let empty = Map::new();
    for (port, input) in &binding.inputs {
        let declaration = &contract.inputs[port];
        let value = match input {
            ApplicationInput::Host { .. } => match options.host_inputs.get(port) {
                Some(value) => value.clone(),
                None if !declaration.required => continue,
                None => {
                    return Err(Diagnostic::new(
                        "missing_input",
                        format!("Missing host input {block}.{port}"),
                        None,
                    ));
                }
            },
            ApplicationInput::Value { value } => value.clone(),
            ApplicationInput::Profile {
                profile: name,
                config,
            } if standard(name) => profile::project(
                name,
                application.model(&instance.model)?,
                state,
                config.as_ref().unwrap_or(&empty),
                &|branch, action| {
                    world.action_applicable(&instance.id, branch, action, &BTreeMap::new())
                },
            )?,
            ApplicationInput::Profile { .. } => {
                if options.basis.is_none() {
                    return Err(error("Custom projections require the captured World basis"));
                }
                options
                    .projections
                    .get(port)
                    .ok_or_else(|| error(format!("Missing custom projection {block}.{port}")))?
                    .clone()
            }
        };
        crate::encoding::validate_value(&value)?;
        inputs.insert(
            port.clone(),
            Record {
                schema: declaration.schema.clone(),
                value,
                basis: basis.clone(),
                sequence: options.sequence,
                unit: declaration.unit.clone(),
                frame: declaration.frame.clone(),
            },
        );
    }
    Ok(Capture {
        instance: instance.id.clone(),
        basis,
        inputs,
        outputs: binding.outputs.clone().unwrap_or_default(),
    })
}
