use std::io::{Read, Write};

use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};

use crate::{Diagnostic, limits::MAX_INPUT_BYTES};

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_archive", message, None)
}

pub fn compress(source: &str) -> Result<Vec<u8>, Diagnostic> {
    crate::limits::input(source, false)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(source.as_bytes())
        .map_err(|error| invalid(error.to_string()))?;
    encoder.finish().map_err(|error| invalid(error.to_string()))
}

pub fn decompress(bytes: &[u8]) -> Result<String, Diagnostic> {
    if bytes.len() > MAX_INPUT_BYTES + 65536 {
        return Err(Diagnostic::new(
            "resource_limit",
            "Compressed archive exceeds the bounded transport budget",
            None,
        ));
    }
    let mut decoder = GzDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .by_ref()
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|error| invalid(error.to_string()))?;
    if output.len() > MAX_INPUT_BYTES {
        return Err(Diagnostic::new(
            "resource_limit",
            "Decoded archive exceeds 8 MiB",
            None,
        ));
    }
    if !decoder.into_inner().is_empty() {
        return Err(invalid(
            "Unexpected trailing bytes or multiple gzip members",
        ));
    }
    String::from_utf8(output).map_err(|error| invalid(error.to_string()))
}
