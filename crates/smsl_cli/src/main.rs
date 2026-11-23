mod abstraction;
mod encoding;
mod exploration;
mod formatting;
mod mechanisms;
mod memory;
mod sources;
mod system;

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use smsl_core::runtime;

use encoding::{convert, read_json, read_machine, serialize};

const USAGE: &str = "State Language (state-lang) — State Machine Serialization Language

Usage:
  smsl fmt [--check] [PATH ... | -]
  smsl check INPUT
  smsl validate INPUT
  smsl convert INPUT --to smsl|json|yaml|xml|toml [--linked] [--output PATH]
  smsl graph INPUT --to json|dot [--output PATH]
  smsl query INPUT BRANCH STATE OPERATION
  smsl abstract MODEL SNAPSHOT BRANCH MAPPING [--to json|yaml|xml|toml]
  smsl concretize MODEL SNAPSHOT BRANCH MAPPING REQUIREMENTS [--knowledge] [--to FORMAT]
  smsl matches MODEL ACTIVE QUERY [CANDIDATE] [--to FORMAT]
  smsl validate-concretization MODEL ACTIVE QUERY
  smsl start INPUT CONTEXT [--to smsl|json|yaml|xml|toml]
  smsl advance INPUT SNAPSHOT EVENT [--to json|yaml|xml|toml]
  smsl reset INPUT SNAPSHOT CONTEXT [--to smsl|json|yaml|xml|toml]
  smsl memory world INPUT --to FORMAT [--output PATH]
  smsl memory snapshot MODEL INPUT --to FORMAT [--output PATH]
  smsl system catalog
  smsl system check INPUT
  smsl system inspect INPUT
  smsl system convert INPUT --to FORMAT [--linked] [--output PATH]
  smsl execution check INPUT
  smsl execution convert INPUT --to FORMAT [--output PATH]
  smsl execution start INPUT WORLD [--output ARCHIVE]
  smsl execution apply ARCHIVE INPUTS --save ARCHIVE [--output REPORTS]
  smsl planning MODEL SNAPSHOT REQUEST
  smsl analyze MODEL [--snapshot PATH] [--operators PATH] [--output PATH]
  smsl explore MODEL [--snapshot PATH] [--operators PATH] [--max-states N] [--output PATH]
  smsl --version

File extensions .json/.yaml/.yml/.xml/.toml select their data encoding.
Other definition files and - use State Language (.smsl). Runtime output defaults to JSON.
Imports resolve relative to their importing file. Conversion preserves imports;
--linked writes a self-contained definition with resolved branches.
Memory FORMAT accepts all five encodings; .gz paths use gzip compression.";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--help" | "-h"] => println!("{USAGE}"),
        ["--version"] => println!(
            "State Language {} (smsl; language {})",
            smsl_core::PACKAGE_VERSION,
            smsl_core::LANGUAGE_VERSION
        ),
        ["fmt", args @ ..] => formatting::run(args)?,
        ["check" | "validate", path] => {
            let machine = read_machine(path)?;
            let branches = &machine.document().branches;
            let monitor_states: usize = branches.iter().map(|branch| branch.states.len()).sum();
            let named_states: usize = branches
                .iter()
                .map(|branch| branch.named_states.len())
                .sum();
            let operations: usize = branches.iter().map(|branch| branch.operations.len()).sum();
            let transitions: usize = branches
                .iter()
                .flat_map(|branch| &branch.states)
                .map(|state| state.transitions.len())
                .sum();
            let reactions: usize = branches
                .iter()
                .map(|branch| {
                    branch.reactions.len()
                        + branch
                            .states
                            .iter()
                            .map(|state| state.reactions.len())
                            .sum::<usize>()
                })
                .sum();
            println!(
                "valid: {} branches, {named_states} computed states, {monitor_states} monitor states, {operations} operations, {transitions} monitor transitions, {reactions} reactions",
                branches.len()
            );
        }
        ["convert", path, "--to", format] => {
            io::stdout().write_all(convert(path, format, false)?.as_bytes())?;
        }
        ["convert", path, "--to", format, "--output", output] => {
            fs::write(output, convert(path, format, false)?)?;
        }
        ["convert", path, "--to", format, "--linked"] => {
            io::stdout().write_all(convert(path, format, true)?.as_bytes())?;
        }
        [
            "convert",
            path,
            "--to",
            format,
            "--linked",
            "--output",
            output,
        ] => {
            fs::write(output, convert(path, format, true)?)?;
        }
        ["graph", path, "--to", format] => {
            io::stdout().write_all(graph(path, format)?.as_bytes())?;
        }
        ["graph", path, "--to", format, "--output", output] => {
            fs::write(output, graph(path, format)?)?;
        }
        ["query", path, branch, state, operation] => {
            let machine = read_machine(path)?;
            let successor = machine.successor(branch, state, operation)?;
            println!(
                "{}",
                serde_json::to_string(&successor.map(|state| &state.name))?
            );
        }
        ["system", args @ ..] => system::run(args)?,
        ["execution", args @ ..] => mechanisms::execution(args)?,
        ["planning", model, snapshot, request] => mechanisms::planning(model, snapshot, request)?,
        [command @ ("analyze" | "explore"), args @ ..] => exploration::run(command, args)?,
        ["memory", args @ ..] => memory::run(args)?,
        [
            command @ ("abstract" | "concretize" | "matches" | "validate-concretization"),
            args @ ..,
        ] => abstraction::run(command, args)?,
        ["start", path, context] => start(path, context, "json")?,
        ["start", path, context, "--to", format] => start(path, context, format)?,
        ["advance", path, snapshot, event] => advance(path, snapshot, event, "json")?,
        ["advance", path, snapshot, event, "--to", format] => {
            advance(path, snapshot, event, format)?
        }
        ["reset", path, snapshot, context] => reset(path, snapshot, context, "json")?,
        ["reset", path, snapshot, context, "--to", format] => {
            reset(path, snapshot, context, format)?
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn start(path: &str, context: &str, format: &str) -> Result<(), Box<dyn Error>> {
    let machine = read_machine(path)?;
    let context = serde_json::from_str(&read_json(context)?)?;
    let snapshot = runtime::initial(&machine, context)?;
    print!("{}", memory::dump_snapshot(&snapshot, format)?);
    Ok(())
}

fn advance(path: &str, snapshot: &str, event: &str, format: &str) -> Result<(), Box<dyn Error>> {
    let machine = read_machine(path)?;
    let snapshot = memory::read_snapshot(&machine, snapshot)?;
    let event = runtime::parse_event(&read_json(event)?)?;
    let result = runtime::advance(&machine, &snapshot, &event)?;
    print!("{}", serialize(&serde_json::to_value(result)?, format)?);
    Ok(())
}

fn reset(path: &str, snapshot: &str, context: &str, format: &str) -> Result<(), Box<dyn Error>> {
    let machine = read_machine(path)?;
    let snapshot = memory::read_snapshot(&machine, snapshot)?;
    let context = serde_json::from_str(&read_json(context)?)?;
    let result = runtime::reset(&machine, &snapshot, context)?;
    print!("{}", memory::dump_snapshot(&result, format)?);
    Ok(())
}

fn graph(path: &str, format: &str) -> Result<String, Box<dyn Error>> {
    if !matches!(format, "json" | "dot") {
        return Err(format!("unsupported graph format {format:?}; use json or dot").into());
    }
    let graph = read_machine(path)?.graph();
    Ok(match format {
        "json" => graph.to_json(),
        _ => graph.to_dot(),
    })
}
