use crate::{boundary::*, *};
use smsl_core::runtime;
use std::sync::Arc;
api!(smsl_snapshot_data(value: *const SmslSnapshot) -> *mut SmslData { data(&get(value)?.state.snapshot()) });
api!(smsl_snapshot_branch(value: *const SmslSnapshot, branch: SmslString) -> *mut SmslData {
    data(get(value)?.state.branch(text(branch)?).ok_or_else(|| absent("Unknown branch"))?)
});
api!(smsl_snapshot_advance(value: *const SmslSnapshot, event: *const SmslEvent) -> *mut SmslStep {
    let value = get(value)?;
    Ok(own(SmslStep { machine: Arc::clone(&value.machine), inner: value.state.advance(&value.machine, &get(event)?.0)? }))
});
api!(smsl_snapshot_confirm(value: *const SmslSnapshot, event: *const SmslEvent, operations: *const SmslNamePair, count: usize) -> *mut SmslStep {
    let value = get(value)?;
    Ok(own(SmslStep { machine: Arc::clone(&value.machine), inner: value.state.confirm(&value.machine, &get(event)?.0, &pairs(operations, count)?)? }))
});
api!(smsl_snapshot_reset(value: *const SmslSnapshot, ctx: *const SmslContext) -> *mut SmslSnapshot {
    let value = get(value)?;
    Ok(own(SmslSnapshot { machine: Arc::clone(&value.machine), state: Arc::new(value.state.reset(&value.machine, context(ctx)?)?) }))
});
api!([EncodingError] smsl_snapshot_dump(value: *const SmslSnapshot, encoding: u32) -> *mut SmslBuffer {
    Ok(own(SmslBuffer(smsl_core::memory::dump_snapshot(&get(value)?.state.snapshot(), source_format(encoding)?)?)))
});
api!(smsl_step_snapshot(value: *const SmslStep) -> *mut SmslSnapshot {
    let value = get(value)?;
    Ok(own(SmslSnapshot { machine: Arc::clone(&value.machine), state: Arc::new(value.inner.state.clone()) }))
});
api!(smsl_step_data(value: *const SmslStep) -> *mut SmslData {
    let value = get(value)?;
    data(&serde_json::json!({"derived_events": value.inner.derived_events, "changed": value.inner.changed, "event_ref": value.inner.event_ref}))
});
api!(smsl_step_event(value: *const SmslStep, index: usize) -> *mut SmslEvent {
    Ok(own(SmslEvent(get(value)?.inner.derived_events.get(index).ok_or_else(|| smsl_core::Diagnostic::new("out_of_range", "Derived event index exceeds step", None))?.clone())))
});
free!(smsl_snapshot_free, SmslSnapshot);
free!(smsl_step_free, SmslStep);

api!(smsl_snapshot_activation(value: *const SmslSnapshot, branch: SmslString) -> *mut SmslData {
    let value = get(value)?;
    data(&value.state.activation(&value.machine, text(branch)?)?)
});

api!(smsl_snapshot_applicable(value: *const SmslSnapshot, branch: SmslString, action: SmslString, arguments: *const SmslData) -> u8 {
    let value = get(value)?;
    Ok(runtime::actions::applicable(&value.machine, &value.state, text(branch)?, text(action)?, &record(arguments)?)?.into())
});
api!(smsl_snapshot_evolve(value: *const SmslSnapshot, change: *const SmslData) -> *mut SmslSnapshot {
    let value = get(value)?;
    let (machine, state) = runtime::evolution::evolve(&value.machine, &value.state, &record(change)?)?;
    Ok(own(SmslSnapshot { machine, state: Arc::new(state) }))
});
api!(smsl_snapshot_definition(value: *const SmslSnapshot) -> *mut SmslDefinition {
    Ok(own(SmslDefinition(get(value)?.machine.document().clone())))
});
