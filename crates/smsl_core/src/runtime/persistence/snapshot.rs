use crate::runtime::{Event, Snapshot, validate_snapshot};

use crate::{Diagnostic, Machine};

pub fn restore(machine: &Machine, source: &str) -> Result<Snapshot, Diagnostic> {
    crate::limits::input(source, false)?;
    let snapshot = serde_json::from_str(source).map_err(|error| {
        Diagnostic::new(
            "invalid_snapshot",
            format!("Invalid snapshot JSON: {error}"),
            None,
        )
    })?;
    validate_snapshot(machine, &snapshot)?;
    check_json_profile(source)?;
    Ok(snapshot)
}

pub fn dump_snapshot(snapshot: &Snapshot) -> Result<String, Diagnostic> {
    let source = serde_json::to_string_pretty(snapshot)
        .map(|source| source + "\n")
        .map_err(|error| Diagnostic::new("invalid_snapshot", error.to_string(), None))?;
    crate::limits::input(&source, false)?;
    check_json_profile(&source)?;
    Ok(source)
}

pub fn parse_event(source: &str) -> Result<Event, Diagnostic> {
    crate::limits::input(source, false)?;
    let event = serde_json::from_str(source).map_err(|error| {
        Diagnostic::new(
            "invalid_event",
            format!("Invalid event JSON: {error}"),
            None,
        )
    })?;
    check_json_profile(source)?;
    Ok(event)
}

pub(crate) fn check_json_profile(source: &str) -> Result<(), Diagnostic> {
    crate::encoding::decode(source, crate::encoding::Encoding::Json).map(|_| ())
}
