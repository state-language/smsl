use crate::{boundary::*, *};
use smsl_core::{Diagnostic, system};

pub enum SmslSystemSession {
    Raw(system::Session),
    Driven(system::ProviderDriver),
}

fn response(source: String) -> Result<*mut SmslData, Diagnostic> {
    let value = serde_json::from_str(&source).map_err(|error| invalid(error.to_string()))?;
    Ok(own(SmslData(value)))
}

api!(smsl_system_request(request: *const SmslData) -> *mut SmslData {
    response(system::request(&get(request)?.0.to_string())?)
});
api!(smsl_system_session_new(definition: *const SmslData, basis: *const SmslData) -> *mut SmslSystemSession {
    Ok(own(SmslSystemSession::Raw(system::Session::new(record(definition)?, record(basis)?)?)))
});
api!(smsl_system_session_driver_new(definition: *const SmslData, basis: *const SmslData, providers: *const SmslData) -> *mut SmslSystemSession {
    Ok(own(SmslSystemSession::Driven(system::ProviderDriver::new(record(definition)?, record(basis)?, record(providers)?)?)))
});
api!(smsl_system_session_request(session: *mut SmslSystemSession, request: *const SmslData) -> *mut SmslData {
    let source = get(request)?.0.to_string();
    response(match get_mut(session)? {
        SmslSystemSession::Raw(session) => session.request(&source)?,
        SmslSystemSession::Driven(driver) => driver.request(&source)?,
    })
});
api!(smsl_system_session_drive(session: *mut SmslSystemSession, request: *const SmslData) -> *mut SmslData {
    let SmslSystemSession::Driven(driver) = get_mut(session)? else {
        return Err(invalid("Session has no provider driver"));
    };
    response(driver.drive(&get(request)?.0.to_string())?)
});
free!(smsl_system_session_free, SmslSystemSession);

pub struct SmslRemoteClient(system::remote::Client);
api!(smsl_remote_client_new(config: *const SmslData) -> *mut SmslRemoteClient {
    Ok(own(SmslRemoteClient(system::remote::Client::new(get(config)?.0.clone())?)))
});
api!(smsl_remote_client_request(client: *mut SmslRemoteClient, request: *const SmslData) -> *mut SmslData {
    response(get_mut(client)?.0.request(&get(request)?.0.to_string())?)
});
free!(smsl_remote_client_free, SmslRemoteClient);
