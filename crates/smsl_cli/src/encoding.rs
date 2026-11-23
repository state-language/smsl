use std::{
    error::Error,
    fs,
    io::{self, Read},
    path::Path,
};

use smsl_core::{
    Machine,
    encoding::{Encoding, decode, encode},
    modules::{self, Module, SourceFormat},
};

use crate::sources::Files;

pub fn read_machine(path: &str) -> Result<Machine, Box<dyn Error>> {
    let mut files = Files::default();
    let source = files.root(path)?;
    modules::link(source, &mut files).map_err(|error| format!("{path}: {error}").into())
}

pub fn convert(path: &str, format: &str, linked: bool) -> Result<String, Box<dyn Error>> {
    let format = format.parse::<SourceFormat>()?;
    let mut files = Files::default();
    let source = files.root(path)?;
    let module = if linked {
        Module::from(modules::link_document(source, &mut files)?)
    } else {
        modules::decode(&source.text, source.format)?
    };
    Ok(modules::encode(&module, format)?)
}

pub fn read_json(path: &str) -> Result<String, Box<dyn Error>> {
    let value = decode(&read(path)?, file_encoding(path))?;
    Ok(serde_json::to_string(&value)?)
}

pub fn serialize(value: &serde_json::Value, format: &str) -> Result<String, Box<dyn Error>> {
    Ok(encode(value, format.parse::<Encoding>()?)?)
}

fn file_encoding(path: &str) -> Encoding {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("yaml" | "yml") => Encoding::Yaml,
        Some("xml") => Encoding::Xml,
        Some("toml") => Encoding::Toml,
        _ => Encoding::Json,
    }
}

fn read(path: &str) -> Result<String, Box<dyn Error>> {
    if path == "-" {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        Ok(source)
    } else {
        Ok(fs::read_to_string(path)?)
    }
}
