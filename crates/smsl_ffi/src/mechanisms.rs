use crate::{boundary::*, *};
use smsl_core::{Diagnostic, execution, planning};

pub struct SmslExecutionSession(pub(crate) execution::Session);

fn response(source: String) -> Result<*mut SmslData, Diagnostic> {
    let value = serde_json::from_str(&source).map_err(|cause| invalid(cause.to_string()))?;
    Ok(own(SmslData(value)))
}

api!(smsl_execution_request(request: *const SmslData) -> *mut SmslData {
    response(execution::request(&get(request)?.0.to_string())?)
});
api!(smsl_execution_session_new(definition: *const SmslData, world: *const SmslWorld) -> *mut SmslExecutionSession {
    Ok(own(SmslExecutionSession(execution::Session::new(record(definition)?, get(world)?.0.clone())?)))
});
api!(smsl_execution_session_restore(source: SmslString, encoding: u32) -> *mut SmslExecutionSession {
    Ok(own(SmslExecutionSession(execution::Session::restore(text(source)?, format(encoding)?)?)))
});
api!(smsl_execution_session_request(session: *mut SmslExecutionSession, request: *const SmslData) -> *mut SmslData {
    response(get_mut(session)?.0.request(&get(request)?.0.to_string())?)
});
api!(smsl_execution_session_world(session: *const SmslExecutionSession) -> *mut SmslWorld {
    Ok(own(SmslWorld(get(session)?.0.world().clone())))
});
free!(smsl_execution_session_free, SmslExecutionSession);

api!(smsl_planning_request(machine: *const SmslMachine, state: *const SmslSnapshot, request: *const SmslData) -> *mut SmslData {
    response(planning::request(&get(machine)?.0, &get(state)?.state, &get(request)?.0.to_string())?)
});
