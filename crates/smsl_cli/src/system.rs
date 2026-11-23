use std::{error::Error, fs};

use smsl_core::{modules::SourceFormat, system};

use crate::sources::Files;

pub fn run(args: &[&str]) -> Result<(), Box<dyn Error>> {
    match args {
        ["catalog"] => println!("{}", serde_json::to_string_pretty(&system::catalog())?),
        ["check" | "validate" | "inspect", path] => {
            let definition = load(path)?;
            println!("{}", serde_json::to_string_pretty(&system::validate(&definition)?)?);
        }
        ["convert", path, "--to", format] => print!("{}", convert(path, format, false)?),
        ["convert", path, "--to", format, "--linked"] => print!("{}", convert(path, format, true)?),
        ["convert", path, "--to", format, "--output", output] => fs::write(output, convert(path, format, false)?)?,
        ["convert", path, "--to", format, "--linked", "--output", output] => fs::write(output, convert(path, format, true)?)?,
        _ => return Err("Usage: smsl system catalog | check INPUT | inspect INPUT | convert INPUT --to FORMAT [--linked] [--output PATH]".into()),
    }
    Ok(())
}

fn load(path: &str) -> Result<system::SystemDefinition, Box<dyn Error>> {
    let mut files = Files::default();
    let source = files.root(path)?;
    Ok(system::link(source, &mut files)?)
}

fn convert(path: &str, format: &str, linked: bool) -> Result<String, Box<dyn Error>> {
    let format: SourceFormat = format.parse()?;
    let definition = if linked {
        load(path)?
    } else {
        let source = Files::default().root(path)?;
        system::decode(&source.text, source.format)?
    };
    Ok(system::encode(&definition, format)?)
}
