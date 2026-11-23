use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::Path,
};

use smsl_core::{Machine, memory, modules::SourceFormat, runtime::Snapshot};

use crate::encoding::read_machine;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn run(args: &[&str]) -> Result<()> {
    let (args, output) = match args {
        [args @ .., "--output", output] => (args, Some(*output)),
        _ => (args, None),
    };
    let text = match args {
        ["world", input, "--to", format] => {
            let (source, input_format) = read(input)?;
            let world = memory::restore_world(&source, input_format)?;
            memory::dump_world(&world, format.parse()?)?
        }
        ["snapshot", model, input, "--to", format] => {
            let machine = read_machine(model)?;
            memory::dump_snapshot(&read_snapshot(&machine, input)?, format.parse()?)?
        }
        _ => return Err(crate::USAGE.into()),
    };
    match output {
        Some(path) if path.ends_with(".gz") => fs::write(path, memory::compress(&text)?)?,
        Some(path) => fs::write(path, text)?,
        None => io::stdout().write_all(text.as_bytes())?,
    }
    Ok(())
}

pub fn read_snapshot(machine: &Machine, path: &str) -> Result<Snapshot> {
    let (source, format) = read(path)?;
    Ok(memory::restore_snapshot(machine, &source, format)?)
}

pub fn dump_snapshot(snapshot: &Snapshot, format: &str) -> Result<String> {
    Ok(memory::dump_snapshot(snapshot, format.parse()?)?)
}

fn read(path: &str) -> Result<(String, SourceFormat)> {
    let (source, logical_path) = match path.strip_suffix(".gz") {
        Some(logical) => (memory::decompress(&fs::read(path)?)?, logical),
        None => (fs::read_to_string(path)?, path),
    };
    let format = match Path::new(logical_path)
        .extension()
        .and_then(|value| value.to_str())
    {
        Some("json") => SourceFormat::Json,
        Some("yaml" | "yml") => SourceFormat::Yaml,
        Some("xml") => SourceFormat::Xml,
        Some("toml") => SourceFormat::Toml,
        _ => SourceFormat::Smsl,
    };
    Ok((source, format))
}
