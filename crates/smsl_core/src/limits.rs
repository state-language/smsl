use crate::diagnostic::Diagnostic;

pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_NESTING: usize = 96;
pub const MAX_TOKENS: usize = 262_144;
pub const MAX_EXPRESSION_DEPTH: usize = 64;
pub const MAX_EXPRESSION_NODES: usize = 4096;
pub const MAX_DECLARATIONS: usize = 100_000;

fn exceeded(message: &str) -> Diagnostic {
    Diagnostic::new("resource_limit", message, None)
}

pub fn input(source: &str, comments: bool) -> Result<(), Diagnostic> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(exceeded("Input exceeds 8 MiB"));
    }
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut comment = false;
    let bytes = source.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        if comment {
            comment = !matches!(byte, b'\r' | b'\n');
        } else if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if comments && byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            comment = true;
        } else if byte == b'"' {
            quoted = true;
        } else if matches!(byte, b'{' | b'[') || (comments && byte == b'(') {
            depth += 1;
            if depth > MAX_NESTING {
                return Err(exceeded("Input nesting exceeds 96 levels"));
            }
        } else if matches!(byte, b'}' | b']') || (comments && byte == b')') {
            depth = depth.saturating_sub(1);
        }
    }
    Ok(())
}
