mod layout;
mod printer;
mod tree;

#[cfg(test)]
mod tests;

use super::lexer::{Kind, tokenize};
use crate::Diagnostic;

/// Format native State Language source without resolving imports or compiling a model.
///
/// Uses four-space indentation and a target line width of 100 characters. Comments,
/// token spelling and declaration order are retained; line endings become LF.
pub fn format(source: &str) -> Result<String, Diagnostic> {
    let tokens = tokenize(source)?;
    if tokens[0].kind == Kind::Name("memory".into()) {
        let kind = match tokens.get(1).map(|token| &token.kind) {
            Some(Kind::Name(kind)) if matches!(kind.as_str(), "snapshot" | "world") => kind,
            _ => {
                return Err(Diagnostic::new(
                    "invalid_memory",
                    "Expected memory snapshot or world",
                    tokens.get(1).map(|token| token.location),
                ));
            }
        };
        crate::memory::validate_native(kind, source)?;
    } else if matches!(tokens.get(3).map(|token| &token.kind), Some(Kind::Name(kind))
        if matches!(kind.as_str(), "system" | "application" | "execution"))
    {
        super::native::parse(source)?;
    } else {
        crate::parse_module(source)?;
    }
    let items = tree::read(source, &tokens);
    let result = printer::render(&layout::document(&items));
    crate::limits::input(&result, true)?;
    Ok(result)
}
