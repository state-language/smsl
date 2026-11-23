use crate::{
    Diagnostic,
    encoding::{self, Encoding},
    limits,
    model::Document,
};

pub fn decode(source: &str, format: Encoding) -> Result<Document, Diagnostic> {
    let json = if format == Encoding::Json {
        limits::input(source, false)?;
        source.to_owned()
    } else {
        serde_json::to_string(&encoding::decode(source, format)?).expect("decoded values")
    };
    let mut document = serde_json::from_str(&json).map_err(|error| {
        Diagnostic::new(
            "syntax",
            format!("Invalid SMSL JSON: {error}"),
            Some(crate::diagnostic::json_location(&json, &error)),
        )
    })?;
    crate::language::limits::document(&mut document)?;
    encoding::decode(&json, Encoding::Json)?;
    Ok(document)
}

pub fn encode(document: &Document, format: Encoding) -> Result<String, Diagnostic> {
    crate::language::limits::check_document(document)?;
    encoding::encode(
        &serde_json::to_value(document).expect("bounded definition"),
        format,
    )
}
