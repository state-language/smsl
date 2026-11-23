use std::{error::Error, fs};

use smsl_core::{
    exploration,
    runtime::{Context, RuntimeState},
};

use crate::{
    encoding::{read_json, read_machine},
    memory,
};

pub fn run(command: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let [model, options @ ..] = args else {
        return Err(crate::USAGE.into());
    };
    let mut snapshot = None;
    let mut operators = None;
    let mut output = None;
    let mut max_states = None;
    for option in options.chunks(2) {
        match option {
            ["--snapshot", path] if snapshot.is_none() => snapshot = Some(*path),
            ["--operators", path] if operators.is_none() => operators = Some(*path),
            ["--output", path] if output.is_none() => output = Some(*path),
            ["--max-states", bound] if command == "explore" && max_states.is_none() => {
                max_states = Some(bound.parse()?)
            }
            _ => return Err(crate::USAGE.into()),
        }
    }
    let machine = read_machine(model)?;
    let state = match snapshot {
        Some(path) => {
            RuntimeState::from_snapshot(&machine, memory::read_snapshot(&machine, path)?)?
        }
        None => RuntimeState::initial(
            &machine,
            Context {
                scope: "exploration".into(),
                subject: "model".into(),
                clock: "logical".into(),
            },
        )?,
    };
    let operators = operators
        .map(|path| read_json(path).and_then(|json| Ok(serde_json::from_str(&json)?)))
        .transpose()?;
    let text = if command == "analyze" {
        serde_json::to_string_pretty(&exploration::analyze(&machine, &state, operators)?)?
    } else {
        serde_json::to_string_pretty(&exploration::explore(
            &machine,
            &state,
            exploration::Options {
                operators,
                max_states: max_states.unwrap_or(100),
            },
        )?)?
    } + "\n";
    match output {
        Some(path) => fs::write(path, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
