use crate::{boundary::*, *};
use smsl_core::{definition, encoding};
api!([CompileError] smsl_definition_from_data(value: *const SmslData) -> *mut SmslDefinition {
    let source = encoding::encode(&get(value)?.0, encoding::Encoding::Json)?;
    Ok(own(SmslDefinition(definition::decode(&source, encoding::Encoding::Json)?)))
});
api!([CompileError] smsl_definition_load(source: SmslString, encoding: u32) -> *mut SmslDefinition {
    let source = text(source)?;
    let value = if encoding == 0 { smsl_core::parse(source)? } else { definition::decode(source, format(encoding)?)? };
    Ok(own(SmslDefinition(value)))
});
api!(smsl_definition_data(value: *const SmslDefinition) -> *mut SmslData { data(&get(value)?.0) });
api!([EncodingError] smsl_definition_dump(value: *const SmslDefinition, encoding: u32) -> *mut SmslBuffer {
    let value = &get(value)?.0;
    let source = if encoding == 0 { smsl_core::to_smsl(&smsl_core::compile(value.clone())?) } else { definition::encode(value, format(encoding)?)? };
    Ok(own(SmslBuffer(source)))
});
api!(smsl_definition_namespace(value: *const SmslDefinition, prefix: SmslString, events: u8) -> *mut SmslDefinition {
    Ok(own(SmslDefinition(definition::namespace(&get(value)?.0, text(prefix)?, boolean(events)?)?)))
});
api!(smsl_definition_map_events(value: *const SmslDefinition, mapping: *const SmslNamePair, count: usize) -> *mut SmslDefinition {
    Ok(own(SmslDefinition(definition::map_events(&get(value)?.0, &pairs(mapping, count)?)?)))
});
api!(smsl_definition_compose(values: *const *const SmslDefinition, count: usize) -> *mut SmslDefinition {
    let values = slice(values, count)?.iter().map(|v| Ok(get(*v)?.0.clone())).collect::<Result<Vec<_>, smsl_core::Diagnostic>>()?;
    Ok(own(SmslDefinition(definition::compose(&values)?)))
});
free!(smsl_definition_free, SmslDefinition);
