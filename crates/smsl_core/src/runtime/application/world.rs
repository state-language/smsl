use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::runtime::{ActionTicket, Event, Snapshot, World, system::Basis};
use crate::{
    Diagnostic,
    model::{Datum, Value},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum KnowledgeUpdate {
    Known { value: Value },
    Unknown {},
    Unchanged {},
}

impl KnowledgeUpdate {
    pub fn parse(source: &str) -> Result<Self, Diagnostic> {
        let value = crate::encoding::decode(source, crate::encoding::Encoding::Json)?;
        serde_json::from_value(value)
            .map_err(|cause| Diagnostic::new("invalid_update", cause.to_string(), None))
    }
}

pub fn world_basis(world: &World, instance: &str) -> Result<Basis, Diagnostic> {
    let state = world.state(instance).ok_or_else(|| {
        Diagnostic::new(
            "invalid_instance",
            format!("Unknown instance {instance:?}"),
            None,
        )
    })?;
    Ok(Basis::from_state(state))
}

pub fn check_basis(world: &World, instance: &str, basis: &Basis) -> Result<(), Diagnostic> {
    if world_basis(world, instance)? != *basis {
        return Err(Diagnostic::new(
            "stale_basis",
            "Provider result has a stale model or knowledge basis",
            None,
        ));
    }
    Ok(())
}

pub fn project_knowledge(
    world: &mut World,
    instance: &str,
    event: &str,
    update: KnowledgeUpdate,
    basis: &Basis,
    time: u64,
) -> Result<Snapshot, Diagnostic> {
    check_basis(world, instance, basis)?;
    let (status, value) = match update {
        KnowledgeUpdate::Unchanged {} => return world.snapshot(instance),
        KnowledgeUpdate::Unknown {} => ("unknown", Datum::Null(())),
        KnowledgeUpdate::Known { value } => (
            "known",
            match value {
                Value::Bool(value) => Datum::Bool(value),
                Value::String(value) => Datum::String(value),
                Value::UInt(value) => Datum::Time(value),
            },
        ),
    };
    let machine = world.machine(instance).expect("checked instance");
    let declaration = machine
        .document()
        .events
        .iter()
        .find(|item| item.name == event)
        .ok_or_else(|| {
            Diagnostic::new(
                "invalid_projection",
                format!("Projection event {event} is not declared"),
                None,
            )
        })?;
    if !["status", "value"]
        .iter()
        .all(|name| declaration.fields.iter().any(|field| field.name == *name))
    {
        return Err(Diagnostic::new(
            "invalid_projection",
            "Projection events require status and value payload fields",
            None,
        ));
    }
    let outcome = Event {
        name: event.into(),
        time,
        context: world
            .state(instance)
            .expect("checked instance")
            .context()
            .clone(),
        cause: None,
        payload: BTreeMap::from([
            ("status".into(), Datum::String(status.into())),
            ("value".into(), value),
        ]),
    };
    Ok(world.advance(instance, &outcome)?.state.snapshot())
}

pub fn begin_action(
    world: &mut World,
    instance: &str,
    branch: &str,
    action: &str,
    arguments: BTreeMap<String, Datum>,
    basis: &Basis,
) -> Result<ActionTicket, Diagnostic> {
    check_basis(world, instance, basis)?;
    world.begin_action(instance, branch, action, arguments)
}

pub fn cancel_action(
    world: &mut World,
    ticket: &ActionTicket,
    acknowledged: bool,
) -> Result<(), Diagnostic> {
    if !acknowledged {
        return Err(Diagnostic::new(
            "cancellation_unacknowledged",
            "External execution must acknowledge stopping first",
            None,
        ));
    }
    world.cancel_action(ticket)
}
