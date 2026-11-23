use crate::{
    Diagnostic, Machine,
    model::{Datum, EventRef},
};

use super::{Context, Event, RuntimeState, SNAPSHOT_VERSION, Snapshot};

pub(crate) use crate::diagnostic::key;

pub(crate) type Resolver<'a> = dyn Fn(&EventRef) -> bool + 'a;

pub(crate) fn error(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message, None)
}

pub(crate) fn context(context: &Context, code: &str) -> Result<(), Diagnostic> {
    for (name, value) in [
        ("scope", &context.scope),
        ("subject", &context.subject),
        ("clock", &context.clock),
    ] {
        if value.is_empty() {
            return Err(
                error(code, "Context fields must be nonempty").at_path(format!("/context/{name}"))
            );
        }
    }
    Ok(())
}

pub(crate) fn reference(
    reference: &EventRef,
    scope: &str,
    step: u64,
    code: &str,
    resolve: &Resolver<'_>,
) -> Result<(), Diagnostic> {
    if reference.step == 0
        || (reference.scope == scope && reference.step > step)
        || (reference.scope != scope && !resolve(reference))
    {
        return Err(error(
            code,
            "Event reference must identify an already consumed local step or retained World event",
        ));
    }
    Ok(())
}

pub(crate) fn event(
    machine: &Machine,
    event: &Event,
    snapshot: &RuntimeState,
    resolve: &Resolver<'_>,
) -> Result<(), Diagnostic> {
    if event.name.is_empty() {
        return Err(error("invalid_event", "Event name must be nonempty").at_path("/event/name"));
    }
    if event.context != snapshot.context {
        return Err(
            error("invalid_event", "Event context must match the snapshot")
                .at_path("/event/context"),
        );
    }
    if snapshot.time.is_some_and(|time| event.time < time) {
        return Err(error(
            "invalid_event",
            "Event time cannot precede the last consumed event",
        )
        .at_path("/event/time"));
    }
    if let Some(cause) = &event.cause {
        reference(
            cause,
            &snapshot.context.scope,
            snapshot.step,
            "invalid_event",
            resolve,
        )
        .map_err(|error| error.at_path("/event/cause"))?;
    }
    let declaration = machine
        .document()
        .events
        .iter()
        .find(|declaration| declaration.name == event.name);
    let fields = declaration.map_or(&[][..], |declaration| declaration.fields.as_slice());
    if fields.len() != event.payload.len() {
        return Err(error(
            "invalid_event",
            "Event payload must contain exactly its declared fields",
        )
        .at_path("/event/payload"));
    }
    for field in fields {
        let path = format!("/event/payload/{}", key(&field.name));
        let value = event.payload.get(&field.name).ok_or_else(|| {
            error("invalid_event", "Missing declared payload field").at_path(&path)
        })?;
        if matches!(value, Datum::Unknown(_))
            || value
                .data_type()
                .map_or(!field.nullable, |_| !field.data_type.accepts(value))
        {
            return Err(error(
                "invalid_event",
                "Incorrect payload field type or nullability",
            )
            .at_path(path));
        }
        if let Datum::Event(value) = value {
            reference(
                value,
                &snapshot.context.scope,
                snapshot.step,
                "invalid_event",
                resolve,
            )
            .map_err(|error| error.at_path(path))?;
        }
    }
    Ok(())
}

pub fn validate_snapshot(machine: &Machine, snapshot: &Snapshot) -> Result<(), Diagnostic> {
    validate_snapshot_with(machine, snapshot, &|_| false)
}

pub(crate) fn validate_snapshot_with(
    machine: &Machine,
    snapshot: &Snapshot,
    resolve: &Resolver<'_>,
) -> Result<(), Diagnostic> {
    let code = "invalid_snapshot";
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(error(code, "Unsupported snapshot format version").at_path("/version"));
    }
    context(&snapshot.context, code)?;
    if snapshot.definition != machine.canonical_source().as_ref()
        || snapshot.revision != machine.revision()
    {
        return Err(error(
            code,
            "Snapshot model revision does not match the compiled machine",
        )
        .at_path("/revision"));
    }
    if (snapshot.step == 0) != snapshot.time.is_none() {
        return Err(error(
            code,
            "Only a snapshot without consumed events has no event time",
        )
        .at_path("/time"));
    }
    let mut state = RuntimeState::from_validated(machine, snapshot.clone());
    validate_values(machine, &state, resolve)?;
    state.recompute(machine)?;
    if state
        .branches
        .iter()
        .any(|(name, current)| current.snapshot() != &snapshot.branches[name])
    {
        return Err(error(
            code,
            "Stored derived facts or labels disagree with their declared expressions",
        )
        .at_path("/branches"));
    }
    state.validate_constraints(machine)?;
    if snapshot.step == 0 && snapshot.generation == 0 {
        let initial = RuntimeState::initial(machine, snapshot.context.clone())?;
        if snapshot.branches != initial.snapshot().branches {
            return Err(error(
                code,
                "An initial snapshot must contain the declared initial values",
            )
            .at_path("/branches"));
        }
    }
    Ok(())
}

pub(crate) fn validate_values(
    machine: &Machine,
    snapshot: &RuntimeState,
    resolve: &Resolver<'_>,
) -> Result<(), Diagnostic> {
    crate::language::semantics::validate_values(machine, &snapshot.branches, &|value| {
        reference(
            value,
            &snapshot.context.scope,
            snapshot.step,
            "invalid_snapshot",
            resolve,
        )
    })
}
