use crate::{boundary::*, *};
use smsl_core::{Diagnostic, runtime};
use std::sync::Arc;

fn snapshot(world: &runtime::World, name: &str) -> Result<SmslSnapshot, Diagnostic> {
    Ok(SmslSnapshot {
        machine: Arc::clone(
            world
                .machine(name)
                .ok_or_else(|| absent("Unknown instance"))?,
        ),
        state: Arc::new(
            world
                .state(name)
                .ok_or_else(|| absent("Unknown instance"))?
                .clone(),
        ),
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn smsl_world_new() -> *mut SmslWorld {
    own(SmslWorld(runtime::World::new()))
}
api!(smsl_world_spawn(value: *mut SmslWorld, name: SmslString, machine: *const SmslMachine, ctx: *const SmslContext) -> *mut SmslSnapshot {
    let world = &mut get_mut(value)?.0;
    let name = text(name)?;
    world.spawn(name, Arc::clone(&get(machine)?.0), context(ctx)?)?;
    Ok(own(snapshot(world, name)?))
});
/// # Safety
/// Pointers must satisfy smsl.h ownership and storage contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_world_remove(
    value: *mut SmslWorld,
    name: SmslString,
    error: *mut *mut SmslError,
) -> SmslStatus {
    unsafe { run(&mut (), error, || get_mut(value)?.0.remove(text(name)?)) }
}
api!(smsl_world_snapshot(value: *const SmslWorld, name: SmslString) -> *mut SmslSnapshot { Ok(own(snapshot(&get(value)?.0, text(name)?)?)) });
api!(smsl_world_advance(value: *mut SmslWorld, name: SmslString, event: *const SmslEvent) -> *mut SmslStep {
    let world = &mut get_mut(value)?.0;
    let name = text(name)?;
    let machine = Arc::clone(world.machine(name).ok_or_else(|| absent("Unknown instance"))?);
    Ok(own(SmslStep { machine, inner: world.advance(name, &get(event)?.0)? }))
});
api!(smsl_world_confirm(value: *mut SmslWorld, name: SmslString, event: *const SmslEvent, operations: *const SmslNamePair, count: usize) -> *mut SmslStep {
    let world = &mut get_mut(value)?.0;
    let name = text(name)?;
    let machine = Arc::clone(world.machine(name).ok_or_else(|| absent("Unknown instance"))?);
    Ok(own(SmslStep { machine, inner: world.confirm(name, &get(event)?.0, &pairs(operations, count)?)? }))
});
api!(smsl_world_reset(value: *mut SmslWorld, name: SmslString, ctx: *const SmslContext) -> *mut SmslSnapshot {
    let world = &mut get_mut(value)?.0;
    let name = text(name)?;
    world.reset(name, context(ctx)?)?;
    Ok(own(snapshot(world, name)?))
});
api!(smsl_world_resolve(value: *const SmslWorld, event_ref: *const SmslEventRef) -> *mut SmslEvent {
    Ok(own(SmslEvent(get(value)?.0.resolve(&reference(event_ref)?).ok_or_else(|| absent("Unknown event reference"))?.clone())))
});
api!(smsl_world_instances(value: *const SmslWorld) -> *mut SmslData { data(&get(value)?.0.instance_names().collect::<Vec<_>>()) });
api!(smsl_world_record_count(value: *const SmslWorld) -> usize { Ok(get(value)?.0.record_count()) });
api!([EncodingError] smsl_world_dump(value: *const SmslWorld, encoding: u32) -> *mut SmslBuffer { Ok(own(SmslBuffer(smsl_core::memory::dump_world(&get(value)?.0, source_format(encoding)?)?))) });
api!(smsl_world_restore(source: SmslString, encoding: u32) -> *mut SmslWorld { Ok(own(SmslWorld(smsl_core::memory::restore_world(text(source)?, source_format(encoding)?)?))) });
free!(smsl_world_free, SmslWorld);
