use std::collections::BTreeMap;

use serde::Deserialize;
use smsl_core::{
    model::{Datum, EventRef},
    runtime::{ActionTicket, Event, evolution::ChangeSet},
};

use crate::{boundary::*, *};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Applicable {
        instance: String,
        branch: String,
        action: String,
        arguments: BTreeMap<String, Datum>,
    },
    BeginAction {
        instance: String,
        branch: String,
        action: String,
        arguments: BTreeMap<String, Datum>,
    },
    CompleteAction {
        ticket: ActionTicket,
        outcome: Event,
    },
    CancelAction {
        ticket: ActionTicket,
    },
    Evolve {
        instance: String,
        change: ChangeSet,
    },
    Definition {
        instance: String,
    },
    ResolveRecord {
        reference: EventRef,
    },
    Counts {},
}

api!(smsl_world_request(value: *mut SmslWorld, request: *const SmslData) -> *mut SmslData {
    let request: Request = record(request)?;
    let world = &mut get_mut(value)?.0;
    match request {
        Request::Applicable { instance, branch, action, arguments } => data(&world.action_applicable(&instance, &branch, &action, &arguments)?),
        Request::BeginAction { instance, branch, action, arguments } => data(&world.begin_action(&instance, &branch, &action, arguments)?),
        Request::CompleteAction { ticket, outcome } => data(&world.complete_action(&ticket, &outcome)?.into_step()),
        Request::CancelAction { ticket } => { world.cancel_action(&ticket)?; data(&()) },
        Request::Evolve { instance, change } => data(&world.evolve(&instance, &change)?.snapshot()),
        Request::Definition { instance } => data(world.machine(&instance).ok_or_else(|| absent("Unknown instance"))?.document()),
        Request::ResolveRecord { reference } => data(world.resolve_record(&reference).ok_or_else(|| absent("Unknown event reference"))?),
        Request::Counts {} => data(&serde_json::json!({"definitions": world.definition_count(), "records": world.record_count(), "pending_actions": world.pending_action_count()})),
    }
});
