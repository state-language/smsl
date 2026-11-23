use crate::{boundary::*, *};
use smsl_core::{encoding, runtime::RuntimeState};
use std::sync::Arc;
api!([CompileError] smsl_machine_load(source: SmslString, encoding: u32) -> *mut SmslMachine {
    let source = text(source)?;
    let machine = if encoding == 0 { smsl_core::load(source)? } else { encoding::from_encoding(source, format(encoding)?)? };
    Ok(own(SmslMachine(Arc::new(machine))))
});
api!([CompileError] smsl_machine_compile(value: *const SmslDefinition) -> *mut SmslMachine { Ok(own(SmslMachine(Arc::new(smsl_core::compile(get(value)?.0.clone())?)))) });
api!(smsl_machine_definition(value: *const SmslMachine) -> *mut SmslDefinition { Ok(own(SmslDefinition(get(value)?.0.document().clone()))) });
api!([EncodingError] smsl_machine_dump(value: *const SmslMachine, encoding: u32) -> *mut SmslBuffer {
    let machine = &get(value)?.0;
    let source = if encoding == 0 { smsl_core::to_smsl(machine) } else { encoding::to_encoding(machine, format(encoding)?)? };
    Ok(own(SmslBuffer(source)))
});
api!(smsl_machine_successor(value: *const SmslMachine, branch: SmslString, state: SmslString, operation: SmslString) -> *mut SmslData {
    data(&get(value)?.0.successor(text(branch)?, text(state)?, text(operation)?)?.map(|v| &v.name))
});
api!(smsl_machine_binding_value(value: *const SmslMachine, branch: SmslString, fact: SmslString, state: SmslString) -> u8 {
    Ok(get(value)?.0.binding_value(text(branch)?, text(fact)?, text(state)?)?.into())
});
api!(smsl_machine_start(value: *const SmslMachine, ctx: *const SmslContext) -> *mut SmslSnapshot {
    let machine = Arc::clone(&get(value)?.0);
    let state = Arc::new(RuntimeState::initial(&machine, context(ctx)?)?);
    Ok(own(SmslSnapshot { machine, state }))
});
api!(smsl_machine_restore(value: *const SmslMachine, source: SmslString, encoding: u32) -> *mut SmslSnapshot {
    let machine = Arc::clone(&get(value)?.0);
    let snapshot = smsl_core::memory::restore_snapshot(&machine, text(source)?, source_format(encoding)?)?;
    let state = Arc::new(RuntimeState::from_snapshot(&machine, snapshot)?);
    Ok(own(SmslSnapshot { machine, state }))
});
api!(smsl_machine_graph(value: *const SmslMachine) -> *mut SmslGraph { Ok(own(SmslGraph(get(value)?.0.graph()))) });
free!(smsl_machine_free, SmslMachine);
