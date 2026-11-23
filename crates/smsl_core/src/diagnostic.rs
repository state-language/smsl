use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub location: Option<Location>,
    pub path: Option<String>,
}

impl Diagnostic {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        location: Option<Location>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location,
            path: None,
        }
    }

    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn prepend_path(mut self, prefix: &str) -> Self {
        self.path = Some(format!("{prefix}{}", self.path.as_deref().unwrap_or("")));
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(location) = self.location {
            write!(formatter, "{}:{}: ", location.line, location.column)?;
        }
        write!(formatter, "{}: {}", self.code, self.message)?;
        if let Some(path) = &self.path {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {}

pub(crate) fn key(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn json_location(source: &str, error: &serde_json::Error) -> Location {
    let start = source
        .split_inclusive('\n')
        .take(error.line().saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let offset = (start + error.column().saturating_sub(1)).min(source.len());
    let column = source[start..]
        .char_indices()
        .take_while(|(index, _)| start + index < offset)
        .count()
        + 1;
    Location {
        offset,
        line: error.line().max(1),
        column,
    }
}
