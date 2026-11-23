use std::{collections::BTreeMap, sync::Arc};

use crate::{boundary::*, *};
use smsl_core::modules::{self, Module, SourceText};

api!([CompileError] smsl_module_decode(source: SmslString, encoding: u32) -> *mut SmslData {
    data(&modules::decode(text(source)?, source_format(encoding)?)?)
});
api!([EncodingError] smsl_module_encode(value: *const SmslData, encoding: u32) -> *mut SmslBuffer {
    let module: Module = record(value)?;
    Ok(own(SmslBuffer(modules::encode(&module, source_format(encoding)?)?)))
});
api!([CompileError] smsl_machine_link(root: SmslString, sources: *const SmslData) -> *mut SmslMachine {
    let sources: BTreeMap<String, SourceText> = record(sources)?;
    Ok(own(SmslMachine(Arc::new(modules::link_sources(text(root)?, &sources)?))))
});
