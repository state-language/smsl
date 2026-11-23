use crate::{boundary::*, *};
use smsl_core::{encoding, runtime};
api!(smsl_event_new(name: SmslString, time: u64, ctx: *const SmslContext, cause: *const SmslEventRef, payload: *const SmslData) -> *mut SmslEvent {
    let event = runtime::Event {
        name: text(name)?.into(), time, context: context(ctx)?,
        cause: if cause.is_null() { None } else { Some(reference(cause)?) },
        payload: serde_json::from_value(get(payload)?.0.clone()).map_err(|e| invalid(e.to_string()))?,
    };
    runtime::codec::encode_event(&event, encoding::Encoding::Json)?;
    Ok(own(SmslEvent(event)))
});
api!(smsl_event_from_data(value: *const SmslData) -> *mut SmslEvent {
    let source = encoding::encode(&get(value)?.0, encoding::Encoding::Json)?;
    Ok(own(SmslEvent(runtime::codec::decode_event(&source, encoding::Encoding::Json)?)))
});
api!(smsl_event_load(source: SmslString, encoding: u32) -> *mut SmslEvent { Ok(own(SmslEvent(runtime::codec::decode_event(text(source)?, format(encoding)?)?))) });
api!(smsl_event_data(value: *const SmslEvent) -> *mut SmslData { data(&get(value)?.0) });
api!([EncodingError] smsl_event_dump(value: *const SmslEvent, encoding: u32) -> *mut SmslBuffer { Ok(own(SmslBuffer(runtime::codec::encode_event(&get(value)?.0, format(encoding)?)?))) });
free!(smsl_event_free, SmslEvent);
