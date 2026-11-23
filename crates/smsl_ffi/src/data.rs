use crate::{boundary::*, *};
use serde_json::Value;
use smsl_core::{Diagnostic, encoding};

#[unsafe(no_mangle)]
pub extern "C" fn smsl_data_null() -> *mut SmslData {
    own(SmslData(Value::Null))
}
#[unsafe(no_mangle)]
pub extern "C" fn smsl_data_u64(value: u64) -> *mut SmslData {
    own(SmslData(Value::from(value)))
}
api!(smsl_data_bool(value: u8) -> *mut SmslData { Ok(own(SmslData(Value::Bool(boolean(value)?)))) });
api!(smsl_data_string(value: SmslString) -> *mut SmslData { checked(Value::String(text(value)?.into())) });
api!(smsl_data_array(values: *const *const SmslData, count: usize) -> *mut SmslData {
    let values = slice(values, count)?.iter().map(|v| Ok(get(*v)?.0.clone())).collect::<Result<Vec<_>, Diagnostic>>()?;
    checked(Value::Array(values))
});
api!(smsl_data_object(entries: *const SmslDataEntry, count: usize) -> *mut SmslData {
    let mut values = serde_json::Map::new();
    for entry in slice(entries, count)? {
        if values.insert(text(entry.key)?.into(), get(entry.value)?.0.clone()).is_some() { return Err(invalid("Duplicate object key")); }
    }
    checked(Value::Object(values))
});
api!(smsl_data_load(source: SmslString, encoding: u32) -> *mut SmslData {
    Ok(own(SmslData(encoding::decode(text(source)?, format(encoding)?)?)))
});
api!([EncodingError] smsl_data_dump(value: *const SmslData, encoding: u32) -> *mut SmslBuffer {
    Ok(own(SmslBuffer(encoding::encode(&get(value)?.0, format(encoding)?)?)))
});
/// # Safety
/// Data must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_data_kind(value: *const SmslData) -> u32 {
    match &unsafe { &*value }.0 {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::String(_) => 2,
        Value::Number(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}
api!(smsl_data_get_bool(value: *const SmslData) -> u8 { get(value)?.0.as_bool().map(u8::from).ok_or_else(|| invalid("Expected Boolean")) });
api!(smsl_data_get_u64(value: *const SmslData) -> u64 { get(value)?.0.as_u64().ok_or_else(|| invalid("Expected unsigned integer")) });
api!(smsl_data_get_string(value: *const SmslData) -> SmslString { get(value)?.0.as_str().map(SmslString::from).ok_or_else(|| invalid("Expected string")) });
api!(smsl_data_count(value: *const SmslData) -> usize {
    match &get(value)?.0 { Value::Array(v) => Ok(v.len()), Value::Object(v) => Ok(v.len()), _ => Err(invalid("Expected array or object")) }
});
api!(smsl_data_at(value: *const SmslData, index: usize) -> *mut SmslData {
    let value = &get(value)?.0;
    let item = match value { Value::Array(v) => v.get(index), Value::Object(v) => v.values().nth(index), _ => return Err(invalid("Expected array or object")) };
    Ok(own(SmslData(item.ok_or_else(|| Diagnostic::new("out_of_range", "Item index exceeds collection", None))?.clone())))
});
api!(smsl_data_key(value: *const SmslData, index: usize) -> SmslString {
    let values = get(value)?.0.as_object().ok_or_else(|| invalid("Expected object"))?;
    values.keys().nth(index).map(|v| v.as_str().into()).ok_or_else(|| Diagnostic::new("out_of_range", "Key index exceeds object", None))
});
api!(smsl_data_get(value: *const SmslData, key: SmslString) -> *mut SmslData {
    let values = get(value)?.0.as_object().ok_or_else(|| invalid("Expected object"))?;
    Ok(own(SmslData(values.get(text(key)?).ok_or_else(|| absent("Unknown object key"))?.clone())))
});
fn checked(value: Value) -> Result<*mut SmslData, Diagnostic> {
    encoding::encode(&value, encoding::Encoding::Json)?;
    Ok(own(SmslData(value)))
}
free!(smsl_data_free, SmslData);

#[unsafe(no_mangle)]
pub extern "C" fn smsl_data_unknown() -> *mut SmslData {
    own(SmslData(serde_json::json!({"kind": "unknown"})))
}
