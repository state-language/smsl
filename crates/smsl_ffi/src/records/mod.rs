mod branches;
mod expressions;
mod types;

pub use types::*;

use crate::{boundary::*, *};
use smsl_core::{Diagnostic, model::*};

pub(super) unsafe fn collect<T, U>(
    values: *const T,
    count: usize,
    convert: impl Fn(&T) -> Result<U, Diagnostic>,
) -> Result<Vec<U>, Diagnostic> {
    unsafe { slice(values, count)? }
        .iter()
        .map(convert)
        .collect()
}
pub(super) unsafe fn value<T: serde::de::DeserializeOwned>(
    data: *const SmslData,
) -> Result<T, Diagnostic> {
    serde_json::from_value(unsafe { get(data)? }.0.clone()).map_err(|e| invalid(e.to_string()))
}
pub(super) fn data_type(kind: u32) -> Result<DataType, Diagnostic> {
    match kind {
        0 => Ok(DataType::Bool),
        1 => Ok(DataType::String),
        2 => Ok(DataType::Time),
        3 => Ok(DataType::Event),
        4 => Ok(DataType::UInt),
        _ => Err(invalid("Unknown data type")),
    }
}

pub(super) unsafe fn optional_text(value: *const SmslString) -> Result<Option<String>, Diagnostic> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(unsafe { text(*get(value)?)? }.into()))
    }
}

pub(super) unsafe fn event_field(record: &SmslEventFieldRecord) -> Result<EventField, Diagnostic> {
    Ok(EventField {
        name: unsafe { text(record.name)? }.into(),
        data_type: data_type(record.data_type)?,
        nullable: boolean(record.nullable)?,
    })
}

api!(smsl_definition_new(record: *const SmslDefinitionRecord) -> *mut SmslDefinition {
    let record = get(record)?;
    let document = Document {
        version: text(record.version)?.into(),
        events: collect(record.events, record.event_count, |event| Ok(EventDeclaration {
            name: text(event.name)?.into(),
            fields: collect(event.fields, event.field_count, |field| event_field(field))?,
        }))?,
        branches: collect(record.branches, record.branch_count, |branch| branches::branch(branch))?,
    };
    smsl_core::definition::encode(&document, smsl_core::encoding::Encoding::Json)?;
    Ok(own(SmslDefinition(document)))
});
