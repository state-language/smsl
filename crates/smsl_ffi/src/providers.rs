use std::{ffi::c_void, ptr};

use smsl_core::{Diagnostic, system};

use crate::{boundary::*, *};

pub type SmslProviderCreate = unsafe extern "C" fn(
    *mut c_void,
    *const SmslData,
    *mut *mut c_void,
    *mut *mut SmslError,
) -> SmslStatus;
pub type SmslProviderInvoke = unsafe extern "C" fn(
    *mut c_void,
    *const SmslData,
    *mut *mut SmslData,
    *mut *mut SmslError,
) -> SmslStatus;
pub type SmslProviderCancel =
    unsafe extern "C" fn(*mut c_void, SmslString, *mut u8, *mut *mut SmslError) -> SmslStatus;
pub type SmslProviderDestroy = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SmslProviderCallbacks {
    pub create: Option<SmslProviderCreate>,
    pub invoke: Option<SmslProviderInvoke>,
    pub cancel: Option<SmslProviderCancel>,
    pub destroy: Option<SmslProviderDestroy>,
}

#[derive(Clone, Copy)]
struct Callbacks {
    create: SmslProviderCreate,
    invoke: SmslProviderInvoke,
    cancel: SmslProviderCancel,
    destroy: SmslProviderDestroy,
}

impl TryFrom<SmslProviderCallbacks> for Callbacks {
    type Error = Diagnostic;
    fn try_from(value: SmslProviderCallbacks) -> Result<Self, Diagnostic> {
        Ok(Self {
            create: value
                .create
                .ok_or_else(|| invalid("Missing provider create callback"))?,
            invoke: value
                .invoke
                .ok_or_else(|| invalid("Missing provider invoke callback"))?,
            cancel: value
                .cancel
                .ok_or_else(|| invalid("Missing provider cancel callback"))?,
            destroy: value
                .destroy
                .ok_or_else(|| invalid("Missing provider destroy callback"))?,
        })
    }
}

struct Provider {
    contracts: Vec<String>,
    context: *mut c_void,
    callbacks: Callbacks,
}

impl system::Provider for Provider {
    fn contracts(&self) -> &[String] {
        &self.contracts
    }
    fn create(
        &self,
        config: &serde_json::Value,
    ) -> Result<Box<dyn system::ProviderSession>, Diagnostic> {
        let config = SmslData(config.clone());
        let mut session = ptr::null_mut();
        let mut error = ptr::null_mut();
        let status =
            unsafe { (self.callbacks.create)(self.context, &config, &mut session, &mut error) };
        callback_result(status, error)?;
        Ok(Box::new(ProviderSession {
            state: session,
            callbacks: self.callbacks,
        }))
    }
}

struct ProviderSession {
    state: *mut c_void,
    callbacks: Callbacks,
}
impl system::ProviderSession for ProviderSession {
    fn invoke(&mut self, call: &system::Invocation) -> Result<system::Records, Diagnostic> {
        let invocation = SmslData(serde_json::to_value(call).expect("provider invocation"));
        let mut output = ptr::null_mut();
        let mut error = ptr::null_mut();
        let status =
            unsafe { (self.callbacks.invoke)(self.state, &invocation, &mut output, &mut error) };
        callback_result(status, error)?;
        if output.is_null() {
            return Err(invalid("Provider returned a null output handle"));
        }
        let output = unsafe { Box::from_raw(output) };
        serde_json::from_value(output.0).map_err(|cause| invalid(cause.to_string()))
    }
    fn cancel(&mut self, request_id: &str) -> Result<(), Diagnostic> {
        let mut acknowledged = 0;
        let mut error = ptr::null_mut();
        let status = unsafe {
            (self.callbacks.cancel)(self.state, request_id.into(), &mut acknowledged, &mut error)
        };
        callback_result(status, error)?;
        if !boolean(acknowledged)? {
            return Err(Diagnostic::new(
                "cancellation_unacknowledged",
                "Provider has not acknowledged stopping",
                None,
            ));
        }
        Ok(())
    }
}
impl Drop for ProviderSession {
    fn drop(&mut self) {
        unsafe { (self.callbacks.destroy)(self.state) };
    }
}

fn callback_result(status: SmslStatus, error: *mut SmslError) -> Result<(), Diagnostic> {
    if !error.is_null() {
        return Err(unsafe { Box::from_raw(error) }.0);
    }
    if status != SmslStatus::Ok {
        return Err(Diagnostic::new(
            "provider_error",
            format!("Provider callback failed with status {}", status as u32),
            None,
        ));
    }
    Ok(())
}

pub struct SmslProviderRegistry(system::ProviderRegistry);
pub struct SmslSystemRuntime(system::Runtime);

#[unsafe(no_mangle)]
pub extern "C" fn smsl_provider_registry_new() -> *mut SmslProviderRegistry {
    own(SmslProviderRegistry(system::ProviderRegistry::default()))
}

api!(smsl_error_new(code: SmslString, message: SmslString) -> *mut SmslError {
    Ok(own(SmslError(Diagnostic::new(text(code)?, text(message)?, None))))
});

api!(smsl_provider_registry_register(registry: *mut SmslProviderRegistry, id: SmslString,
    contracts: *const SmslString, contract_count: usize, context: *mut c_void,
    callbacks: *const SmslProviderCallbacks) -> u8 {
    let contracts = slice(contracts, contract_count)?.iter().map(|value| Ok(text(*value)?.to_owned()))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if contracts.is_empty() || contracts.iter().any(|value| value.trim().is_empty()) {
        return Err(invalid("Provider requires nonempty contract identities"));
    }
    let provider = Provider { contracts, context, callbacks: (*get(callbacks)?).try_into()? };
    get_mut(registry)?.0.register(text(id)?, provider)?;
    Ok(1)
});

api!(smsl_provider_registry_start(registry: *const SmslProviderRegistry, definition: *const SmslData,
    basis: *const SmslData) -> *mut SmslSystemRuntime {
    Ok(own(SmslSystemRuntime(get(registry)?.0.start(record(definition)?, record(basis)?)?)))
});

api!(smsl_system_runtime_invoke(runtime: *mut SmslSystemRuntime, block: SmslString,
    request_id: SmslString, inputs: *const SmslData, allow_effects: u8) -> *mut SmslData {
    data(&get_mut(runtime)?.0.invoke(text(block)?, text(request_id)?, record(inputs)?, boolean(allow_effects)?)?)
});

api!(smsl_system_runtime_request(runtime: *mut SmslSystemRuntime, request: *const SmslData) -> *mut SmslData {
    let source = get_mut(runtime)?.0.request(&get(request)?.0.to_string())?;
    Ok(own(SmslData(serde_json::from_str(&source).expect("session response"))))
});

api!(smsl_system_runtime_cancel(runtime: *mut SmslSystemRuntime, block: SmslString, request_id: SmslString) -> u8 {
    get_mut(runtime)?.0.cancel(text(block)?, text(request_id)?)?;
    Ok(1)
});

api!(smsl_system_runtime_close(runtime: *mut SmslSystemRuntime) -> u8 {
    get_mut(runtime)?.0.close()?;
    Ok(1)
});

free!(smsl_provider_registry_free, SmslProviderRegistry);
free!(smsl_system_runtime_free, SmslSystemRuntime);
