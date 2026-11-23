use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::encoding::Encoding;
use crate::runtime::{RuntimeState, actions, codec};
use crate::{Diagnostic, Machine, application::validate_profile};

pub fn project_profile(
    profile: &str,
    machine: &Machine,
    state: &RuntimeState,
    config: &Map<String, Value>,
) -> Result<Value, Diagnostic> {
    project(profile, machine, state, config, &|branch, action| {
        actions::applicable(machine, state, branch, action, &BTreeMap::new())
    })
}

pub(super) fn project(
    profile: &str,
    machine: &Machine,
    state: &RuntimeState,
    config: &Map<String, Value>,
    applicable: &dyn Fn(&str, &str) -> Result<bool, Diagnostic>,
) -> Result<Value, Diagnostic> {
    state.check_machine(machine)?;
    validate_profile(profile, machine, config)?;
    match profile {
        "snapshot.v1" => {
            Ok(json!({"snapshot": codec::encode_snapshot(&state.snapshot(), Encoding::Json)?}))
        }
        "branch.knowledge.v1" => {
            let branch = config["branch"].as_str().expect("validated branch");
            let declaration =
                &machine.document().branches[machine.branch_id(branch).expect("validated branch")];
            let mut available = Vec::new();
            for action in &declaration.actions {
                if applicable(branch, &action.name)? {
                    available.push(&action.name);
                }
            }
            Ok(json!({
                "facts": state.branch(branch).expect("validated branch").facts,
                "availableActions": available,
            }))
        }
        _ => unreachable!("validated standard profile"),
    }
}
