use crate::*;
use serde::Serialize;
use smsl_core::{Diagnostic, encoding::Encoding, model::EventRef, runtime::Context};
use std::{collections::BTreeMap, mem::size_of};

pub fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_argument", message, None)
}
pub fn absent(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("unknown_reference", message, None)
}
pub fn own<T>(value: T) -> *mut T {
    Box::into_raw(Box::new(value))
}
pub unsafe fn get<'a, T>(value: *const T) -> Result<&'a T, Diagnostic> {
    unsafe { value.as_ref() }.ok_or_else(|| invalid("Required handle is null"))
}
pub unsafe fn get_mut<'a, T>(value: *mut T) -> Result<&'a mut T, Diagnostic> {
    unsafe { value.as_mut() }.ok_or_else(|| invalid("Required handle is null"))
}
pub unsafe fn slice<'a, T>(values: *const T, count: usize) -> Result<&'a [T], Diagnostic> {
    if count == 0 {
        return Ok(&[]);
    }
    if values.is_null() || count > isize::MAX as usize / size_of::<T>() {
        return Err(invalid("Invalid array pointer or length"));
    }
    Ok(unsafe { std::slice::from_raw_parts(values, count) })
}
pub unsafe fn text<'a>(value: SmslString) -> Result<&'a str, Diagnostic> {
    std::str::from_utf8(unsafe { slice(value.data, value.len)? })
        .map_err(|_| invalid("Expected valid UTF-8"))
}
pub unsafe fn context(value: *const SmslContext) -> Result<Context, Diagnostic> {
    let value = unsafe { get(value)? };
    Ok(Context {
        scope: unsafe { text(value.scope)? }.into(),
        subject: unsafe { text(value.subject)? }.into(),
        clock: unsafe { text(value.clock)? }.into(),
    })
}
pub unsafe fn reference(value: *const SmslEventRef) -> Result<EventRef, Diagnostic> {
    let value = unsafe { get(value)? };
    Ok(EventRef {
        scope: unsafe { text(value.scope)? }.into(),
        step: value.step,
    })
}
pub unsafe fn pairs(
    values: *const SmslNamePair,
    count: usize,
) -> Result<BTreeMap<String, String>, Diagnostic> {
    let mut result = BTreeMap::new();
    for pair in unsafe { slice(values, count)? } {
        let name = unsafe { text(pair.name)? }.to_owned();
        if result
            .insert(name, unsafe { text(pair.value)? }.to_owned())
            .is_some()
        {
            return Err(invalid("Duplicate mapping key"));
        }
    }
    Ok(result)
}
pub fn boolean(value: u8) -> Result<bool, Diagnostic> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid("Boolean must be 0 or 1")),
    }
}
pub fn format(value: u32) -> Result<Encoding, Diagnostic> {
    match value {
        1 => Ok(Encoding::Json),
        2 => Ok(Encoding::Yaml),
        3 => Ok(Encoding::Xml),
        4 => Ok(Encoding::Toml),
        _ => Err(invalid("Unknown data format")),
    }
}
pub fn data(value: &impl Serialize) -> Result<*mut SmslData, Diagnostic> {
    serde_json::to_value(value)
        .map(|v| own(SmslData(v)))
        .map_err(|e| invalid(e.to_string()))
}
pub unsafe fn run<T>(
    out: *mut T,
    error: *mut *mut SmslError,
    action: impl FnOnce() -> Result<T, Diagnostic>,
) -> SmslStatus {
    unsafe { run_as(out, error, None, action) }
}
pub unsafe fn run_as<T>(
    out: *mut T,
    error: *mut *mut SmslError,
    status: Option<SmslStatus>,
    action: impl FnOnce() -> Result<T, Diagnostic>,
) -> SmslStatus {
    let result = if out.is_null() {
        Err(invalid("Output pointer is null"))
    } else {
        action()
    };
    match result {
        Ok(value) => {
            unsafe { out.write(value) };
            SmslStatus::Ok
        }
        Err(diagnostic) => {
            let status = if diagnostic.code == "invalid_argument" {
                SmslStatus::InvalidArgument
            } else if let Some(status) = status {
                status
            } else {
                match diagnostic.code.as_str() {
                    "invalid_argument" => SmslStatus::InvalidArgument,
                    "unknown_reference" => SmslStatus::NotFound,
                    "out_of_range" => SmslStatus::OutOfRange,
                    "syntax" | "unknown_version" => SmslStatus::CompileError,
                    "invalid_encoding" => SmslStatus::EncodingError,
                    _ => SmslStatus::RuntimeError,
                }
            };
            if !error.is_null() {
                unsafe { error.write(own(SmslError(diagnostic))) };
            }
            status
        }
    }
}

pub fn source_format(value: u32) -> Result<smsl_core::modules::SourceFormat, Diagnostic> {
    use smsl_core::modules::SourceFormat;
    match value {
        0 => Ok(SourceFormat::Smsl),
        1 => Ok(SourceFormat::Json),
        2 => Ok(SourceFormat::Yaml),
        3 => Ok(SourceFormat::Xml),
        4 => Ok(SourceFormat::Toml),
        _ => Err(invalid("Unknown source format")),
    }
}

pub unsafe fn record<T: serde::de::DeserializeOwned>(
    value: *const SmslData,
) -> Result<T, Diagnostic> {
    let source = smsl_core::encoding::encode(&unsafe { get(value)? }.0, Encoding::Json)?;
    serde_json::from_str(&source).map_err(|error| invalid(error.to_string()))
}
