use std::error::Error;

use smsl_core::{
    Machine,
    runtime::{RuntimeState, abstraction::Concretization},
};

use crate::{
    encoding::{read_json, read_machine, serialize},
    memory::read_snapshot,
};

pub fn run(command: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let (args, format) = match args {
        [args @ .., "--to", format] => (args, *format),
        _ => (args, "json"),
    };
    let [model, snapshot, rest @ ..] = args else {
        return Err(crate::USAGE.into());
    };
    let machine = read_machine(model)?;
    let state = read_state(&machine, snapshot)?;
    let result = match (command, rest) {
        ("abstract", [branch, name]) => {
            serde_json::to_value(state.abstraction(&machine, branch, name)?)?
        }
        ("concretize", [branch, name, requirements]) => serde_json::to_value(state.concretize(
            &machine,
            branch,
            name,
            &serde_json::from_str(&read_json(requirements)?)?,
            false,
        )?)?,
        ("concretize", [branch, name, requirements, "--knowledge"]) => {
            serde_json::to_value(state.concretize(
                &machine,
                branch,
                name,
                &serde_json::from_str(&read_json(requirements)?)?,
                true,
            )?)?
        }
        ("matches", [query]) => {
            let query: Concretization = serde_json::from_str(&read_json(query)?)?;
            serde_json::to_value(state.matches_concretization(&machine, &query, None)?)?
        }
        ("matches", [query, candidate]) => {
            let query: Concretization = serde_json::from_str(&read_json(query)?)?;
            let candidate = read_state(&machine, candidate)?;
            serde_json::to_value(state.matches_concretization(
                &machine,
                &query,
                Some(&candidate),
            )?)?
        }
        ("validate-concretization", [query]) => {
            state.validate_concretization(&machine, &serde_json::from_str(&read_json(query)?)?)?;
            serde_json::Value::Bool(true)
        }
        _ => return Err(crate::USAGE.into()),
    };
    print!("{}", serialize(&result, format)?);
    Ok(())
}

fn read_state(machine: &Machine, path: &str) -> Result<RuntimeState, Box<dyn Error>> {
    Ok(RuntimeState::from_snapshot(
        machine,
        read_snapshot(machine, path)?,
    )?)
}
