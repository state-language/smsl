use crate::diagnostic::Diagnostic;
use crate::language::{Machine, compile};

pub fn from_json(source: &str) -> Result<Machine, Diagnostic> {
    crate::limits::input(source, false)?;
    let value = crate::encoding::decode(source, crate::encoding::Encoding::Json)?;
    if ["templates", "instances", "imports", "exports", "domains"]
        .iter()
        .any(|key| value.get(key).is_some())
    {
        use crate::modules::{SourceFormat, SourceText};
        return crate::modules::link_sources(
            "model.smsl",
            &std::collections::BTreeMap::from([(
                "model.smsl".into(),
                SourceText {
                    text: source.into(),
                    format: SourceFormat::Json,
                },
            )]),
        );
    }
    let document = serde_json::from_str(source).map_err(|error| {
        Diagnostic::new(
            "syntax",
            format!("Invalid SMSL JSON: {error}"),
            Some(crate::diagnostic::json_location(source, &error)),
        )
    })?;
    compile(document)
}

pub fn to_json(machine: &Machine) -> String {
    serde_json::to_string_pretty(machine.document()).expect("SMSL contains only JSON values") + "\n"
}
