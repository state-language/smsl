use crate::{
    encoding::{read_json, read_machine},
    memory,
    sources::Files,
};
use smsl_core::{execution, planning, runtime::RuntimeState};
use std::{error::Error, fs};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn execution(args: &[&str]) -> Result<()> {
    let (args, output) = match args {
        [args @ .., "--output", output] => (args, Some(*output)),
        _ => (args, None),
    };
    let text = match args {
        ["check" | "validate", input] => {
            let definition = definition(input)?;
            format!("valid: {} execution blocks\n", definition.blocks.len())
        }
        ["convert", input, "--to", format] => {
            execution::encode_source(&definition(input)?, format.parse()?)?
        }
        ["start", input, world] => {
            let source = Files::default().root(world)?;
            let world = smsl_core::memory::restore_world(&source.text, source.format)?;
            execution::Session::new(definition(input)?, world)?
                .save(smsl_core::encoding::Encoding::Json)?
        }
        ["apply", archive, inputs, "--save", target] => {
            let source = Files::default().root(archive)?;
            let format = source
                .format
                .encoding()
                .ok_or("Execution archives use json, yaml, xml or toml")?;
            let mut session = execution::Session::restore(&source.text, format)?;
            let inputs: Vec<execution::Input> = serde_json::from_str(&read_json(inputs)?)?;
            let reports = inputs
                .into_iter()
                .map(|input| session.apply(input))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let archive = session.save(smsl_core::encoding::Encoding::Json)?;
            fs::write(target, archive)?;
            serde_json::to_string_pretty(&reports)? + "\n"
        }
        _ => return Err(crate::USAGE.into()),
    };
    if let Some(output) = output {
        fs::write(output, text)?;
    } else {
        print!("{text}");
    }
    Ok(())
}

pub fn planning(model: &str, snapshot: &str, request: &str) -> Result<()> {
    let machine = read_machine(model)?;
    let state = RuntimeState::from_snapshot(&machine, memory::read_snapshot(&machine, snapshot)?)?;
    println!(
        "{}",
        planning::request(&machine, &state, &read_json(request)?)?
    );
    Ok(())
}

fn definition(path: &str) -> Result<execution::Definition> {
    let source = Files::default().root(path)?;
    Ok(execution::decode_source(&source.text, source.format)?)
}
