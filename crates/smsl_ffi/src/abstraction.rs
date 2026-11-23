use crate::{boundary::*, *};
use smsl_core::runtime::abstraction::{AbstractionValue, Concretization};

api!(smsl_abstraction_record(value: *const SmslData, kind: u32) -> *mut SmslData {
    match kind {
        0 => data(&record::<AbstractionValue>(value)?),
        1 => {
            let query: Concretization = record(value)?;
            query.validate_record()?;
            data(&query)
        },
        _ => Err(invalid("Unknown abstraction record kind")),
    }
});

api!(smsl_snapshot_abstraction(value: *const SmslSnapshot, branch: SmslString, name: SmslString) -> *mut SmslData {
    let value = get(value)?;
    data(&value.state.abstraction(&value.machine, text(branch)?, text(name)?)?)
});

api!(smsl_snapshot_concretize(value: *const SmslSnapshot, branch: SmslString, name: SmslString, requirements: *const SmslData, knowledge: u8) -> *mut SmslData {
    let value = get(value)?;
    data(&value.state.concretize(&value.machine, text(branch)?, text(name)?, &record(requirements)?, boolean(knowledge)?)?)
});

/// # Safety
/// Pointers must satisfy smsl.h ownership and storage contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_snapshot_validate_concretization(
    value: *const SmslSnapshot,
    query: *const SmslData,
    error: *mut *mut SmslError,
) -> SmslStatus {
    unsafe {
        run(&mut (), error, || {
            let value = get(value)?;
            value
                .state
                .validate_concretization(&value.machine, &record(query)?)
        })
    }
}

api!(smsl_snapshot_matches_concretization(value: *const SmslSnapshot, query: *const SmslData, candidate: *const SmslSnapshot) -> *mut SmslData {
    let value = get(value)?;
    let candidate = if candidate.is_null() { None } else { Some(get(candidate)?.state.as_ref()) };
    data(&value.state.matches_concretization(&value.machine, &record(query)?, candidate)?)
});
