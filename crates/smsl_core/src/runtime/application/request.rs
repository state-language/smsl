use std::collections::BTreeMap;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::runtime::{ActionTicket, RuntimeState, World, system::Basis};
use crate::{
    Diagnostic, Machine,
    application::ApplicationDefinition,
    encoding::{self, Encoding},
    model::Datum,
};

use super::{CaptureOptions, KnowledgeUpdate, error};

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    ValidateWorld {},
    CapturePlan {
        block: String,
    },
    Capture {
        block: String,
        #[serde(default)]
        host_inputs: BTreeMap<String, Value>,
        #[serde(default = "initial_sequence")]
        sequence: u64,
        #[serde(default)]
        projections: BTreeMap<String, Value>,
        #[serde(default)]
        basis: Option<Basis>,
    },
}

fn initial_sequence() -> u64 {
    1
}

pub fn request(
    application: &ApplicationDefinition,
    world: &World,
    source: &str,
) -> Result<String, Diagnostic> {
    let value = match parse(source)? {
        Request::ValidateWorld {} => {
            super::validate_world(application, world)?;
            Value::Null
        }
        Request::CapturePlan { block } => {
            serde_json::to_value(super::capture_plan(application, world, &block)?)
                .expect("application capture plan")
        }
        Request::Capture {
            block,
            host_inputs,
            sequence,
            projections,
            basis,
        } => serde_json::to_value(super::capture(
            application,
            world,
            &CaptureOptions {
                block,
                host_inputs,
                sequence,
                projections,
                basis,
            },
        )?)
        .expect("application capture"),
    };
    encoding::encode(&value, Encoding::Json)
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum WorldRequest {
    Basis {
        instance: String,
    },
    CheckBasis {
        instance: String,
        basis: Basis,
    },
    ProjectKnowledge {
        instance: String,
        event: String,
        update: KnowledgeUpdate,
        basis: Basis,
        time: u64,
    },
    BeginAction {
        instance: String,
        branch: String,
        action: String,
        arguments: BTreeMap<String, Datum>,
        basis: Basis,
    },
    CancelAction {
        ticket: ActionTicket,
        acknowledged: bool,
    },
}

pub fn world_request(world: &mut World, source: &str) -> Result<String, Diagnostic> {
    let value = match parse(source)? {
        WorldRequest::Basis { instance } => {
            serde_json::to_value(super::world_basis(world, &instance)?)
        }
        WorldRequest::CheckBasis { instance, basis } => {
            super::check_basis(world, &instance, &basis)?;
            Ok(Value::Null)
        }
        WorldRequest::ProjectKnowledge {
            instance,
            event,
            update,
            basis,
            time,
        } => serde_json::to_value(super::project_knowledge(
            world, &instance, &event, update, &basis, time,
        )?),
        WorldRequest::BeginAction {
            instance,
            branch,
            action,
            arguments,
            basis,
        } => serde_json::to_value(super::begin_action(
            world, &instance, &branch, &action, arguments, &basis,
        )?),
        WorldRequest::CancelAction {
            ticket,
            acknowledged,
        } => {
            super::cancel_action(world, &ticket, acknowledged)?;
            Ok(Value::Null)
        }
    }
    .expect("application World result");
    encoding::encode(&value, Encoding::Json)
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum ProfileRequest {
    ProjectProfile {
        profile: String,
        #[serde(default)]
        config: Map<String, Value>,
    },
}

pub fn profile_request(
    machine: &Machine,
    state: &RuntimeState,
    source: &str,
) -> Result<String, Diagnostic> {
    let ProfileRequest::ProjectProfile { profile, config } = parse(source)?;
    encoding::encode(
        &super::project_profile(&profile, machine, state, &config)?,
        Encoding::Json,
    )
}

fn parse<T: DeserializeOwned>(source: &str) -> Result<T, Diagnostic> {
    serde_json::from_value(encoding::decode(source, Encoding::Json)?)
        .map_err(|cause| error(cause.to_string()))
}
