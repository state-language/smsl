use crate::{boundary::*, *};

api!(smsl_application_request(request: *const SmslData) -> *mut SmslData {
    let response = smsl_core::application::request(&get(request)?.0.to_string())?;
    let value = serde_json::from_str(&response).map_err(|error| invalid(error.to_string()))?;
    Ok(own(SmslData(value)))
});

pub struct SmslApplication(std::sync::Arc<smsl_core::application::ApplicationDefinition>);
pub struct SmslLifecycleBinding(smsl_core::runtime::application::LifecycleBinding);

fn runtime_response(source: String) -> Result<*mut SmslData, smsl_core::Diagnostic> {
    Ok(own(SmslData(
        serde_json::from_str(&source).expect("core runtime response"),
    )))
}

api!(smsl_application_compile(request: *const SmslData) -> *mut SmslApplication {
    Ok(own(SmslApplication(std::sync::Arc::new(smsl_core::application::compile_request(&get(request)?.0.to_string())?))))
});
api!(smsl_application_clone(application: *const SmslApplication) -> *mut SmslApplication {
    Ok(own(SmslApplication(std::sync::Arc::clone(&get(application)?.0))))
});
api!(smsl_application_resolved(application: *const SmslApplication) -> *mut SmslData {
    Ok(own(SmslData(serde_json::to_value(get(application)?.0.resolved()).expect("resolved application"))))
});
api!(smsl_application_world(application: *const SmslApplication) -> *mut SmslWorld {
    Ok(own(SmslWorld(smsl_core::runtime::application::create_world(&get(application)?.0)?)))
});
api!(smsl_application_runtime_request(application: *const SmslApplication, world: *const SmslWorld, request: *const SmslData) -> *mut SmslData {
    runtime_response(smsl_core::runtime::application::request(&get(application)?.0, &get(world)?.0, &get(request)?.0.to_string())?)
});
api!(smsl_application_lifecycle_new(application: *const SmslApplication, world: *const SmslWorld, block: SmslString) -> *mut SmslLifecycleBinding {
    Ok(own(SmslLifecycleBinding(smsl_core::runtime::application::LifecycleBinding::new(&get(application)?.0, &get(world)?.0, text(block)?)?)))
});
api!(smsl_application_lifecycle_cursor(binding: *const SmslLifecycleBinding) -> *mut SmslData {
    data(&get(binding)?.0.cursor())
});
api!(smsl_application_lifecycle_apply(binding: *mut SmslLifecycleBinding, world: *mut SmslWorld, page: *const SmslData, time: u64) -> *mut SmslData {
    let page = serde_json::from_value(get(page)?.0.clone()).map_err(|cause| invalid(cause.to_string()))?;
    data(&get_mut(binding)?.0.apply(&mut get_mut(world)?.0, page, time)?)
});
api!(smsl_application_lifecycle_apply_execution(binding: *mut SmslLifecycleBinding, session: *mut SmslExecutionSession, page: *const SmslData, time: u64) -> *mut SmslData {
    data(&get_mut(binding)?.0.apply_execution(&mut get_mut(session)?.0, record(page)?, time)?)
});
api!(smsl_world_runtime_request(world: *mut SmslWorld, request: *const SmslData) -> *mut SmslData {
    runtime_response(smsl_core::runtime::application::world_request(&mut get_mut(world)?.0, &get(request)?.0.to_string())?)
});
api!(smsl_snapshot_profile_request(snapshot: *const SmslSnapshot, request: *const SmslData) -> *mut SmslData {
    let snapshot = get(snapshot)?;
    runtime_response(smsl_core::runtime::application::profile_request(&snapshot.machine, &snapshot.state, &get(request)?.0.to_string())?)
});
free!(smsl_application_free, SmslApplication);
free!(smsl_application_lifecycle_free, SmslLifecycleBinding);
