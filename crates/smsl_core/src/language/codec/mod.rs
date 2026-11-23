pub(crate) mod json;

use super::Machine;
use crate::{
    Diagnostic,
    encoding::{Encoding, decode, encode},
};
pub use json::{from_json, to_json};

pub fn from_encoding(source: &str, encoding: Encoding) -> Result<Machine, Diagnostic> {
    if encoding == Encoding::Json {
        return json::from_json(source);
    }
    json::from_json(&serde_json::to_string(&decode(source, encoding)?).expect("decoded values"))
}

pub fn to_encoding(machine: &Machine, encoding: Encoding) -> Result<String, Diagnostic> {
    encode(
        &serde_json::to_value(machine.document()).expect("validated definition"),
        encoding,
    )
}
