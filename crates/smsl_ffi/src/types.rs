use smsl_core::{Diagnostic, Machine, graph, model, runtime};
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SmslStatus {
    Ok = 0,
    InvalidArgument = 1,
    CompileError = 2,
    NotFound = 3,
    OutOfRange = 4,
    NoTransition = 5,
    RuntimeError = 6,
    EncodingError = 7,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SmslString {
    pub data: *const u8,
    pub len: usize,
}
impl From<&str> for SmslString {
    fn from(value: &str) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }
}
#[repr(C)]
pub struct SmslDiagnostic {
    pub code: SmslString,
    pub message: SmslString,
    pub path: SmslString,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub has_location: u8,
    pub has_path: u8,
}
#[repr(C)]
pub struct SmslContext {
    pub scope: SmslString,
    pub subject: SmslString,
    pub clock: SmslString,
}
#[repr(C)]
pub struct SmslEventRef {
    pub scope: SmslString,
    pub step: u64,
}
#[repr(C)]
pub struct SmslDataEntry {
    pub key: SmslString,
    pub value: *const SmslData,
}
#[repr(C)]
pub struct SmslNamePair {
    pub name: SmslString,
    pub value: SmslString,
}
pub struct SmslError(pub(crate) Diagnostic);
pub struct SmslBuffer(pub(crate) String);
pub struct SmslData(pub(crate) serde_json::Value);
pub struct SmslDefinition(pub(crate) model::Document);
pub struct SmslMachine(pub(crate) Arc<Machine>);
pub struct SmslEvent(pub(crate) runtime::Event);
#[derive(Clone)]
pub struct SmslSnapshot {
    pub(crate) machine: Arc<Machine>,
    pub(crate) state: Arc<runtime::RuntimeState>,
}
pub struct SmslStep {
    pub(crate) machine: Arc<Machine>,
    pub(crate) inner: runtime::RuntimeStep,
}
pub struct SmslWorld(pub(crate) runtime::World);
pub struct SmslGraph(pub(crate) graph::Graph);

#[unsafe(no_mangle)]
pub extern "C" fn smsl_abi_version() -> u32 {
    1
}

/// # Safety
/// Both arguments must point to live handles/storage. Views last until error is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_error_diagnostic(error: *const SmslError, out: *mut SmslDiagnostic) {
    let error = &unsafe { &*error }.0;
    unsafe {
        out.write(SmslDiagnostic {
            code: error.code.as_str().into(),
            message: error.message.as_str().into(),
            path: error.path.as_deref().unwrap_or("").into(),
            offset: error.location.map_or(0, |v| v.offset),
            line: error.location.map_or(0, |v| v.line),
            column: error.location.map_or(0, |v| v.column),
            has_location: error.location.is_some().into(),
            has_path: error.path.is_some().into(),
        });
    }
}
/// # Safety
/// Buffer must be live. The view remains valid until the buffer is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_buffer_view(buffer: *const SmslBuffer) -> SmslString {
    unsafe { &*buffer }.0.as_str().into()
}
free!(smsl_error_free, SmslError);
free!(smsl_buffer_free, SmslBuffer);
