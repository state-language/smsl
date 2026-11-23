use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{ApplicationProfileDeclaration, error, validation::name};
use crate::{Diagnostic, language::Machine};

pub(super) fn declarations(
    profiles: &[ApplicationProfileDeclaration],
) -> Result<BTreeMap<String, String>, Diagnostic> {
    let mut result = BTreeMap::from([
        ("snapshot.v1".into(), "state".into()),
        ("branch.knowledge.v1".into(), "knowledge".into()),
    ]);
    for profile in profiles {
        name(&profile.id, "Profile id")?;
        name(&profile.schema, "Profile schema")?;
        if result
            .insert(profile.id.clone(), profile.schema.clone())
            .is_some()
        {
            return Err(error(format!("Duplicate profile {}", profile.id)));
        }
    }
    Ok(result)
}

pub fn validate_profile(
    profile: &str,
    machine: &Machine,
    config: &Map<String, Value>,
) -> Result<(), Diagnostic> {
    match profile {
        "snapshot.v1" if config.is_empty() => Ok(()),
        "snapshot.v1" => Err(error("snapshot.v1 config must be empty")),
        "branch.knowledge.v1" => {
            let branch = config
                .get("branch")
                .and_then(Value::as_str)
                .filter(|branch| !branch.trim().is_empty())
                .filter(|_| config.len() == 1)
                .ok_or_else(|| {
                    error("branch.knowledge.v1 config requires exactly a nonempty branch")
                })?;
            let branch = machine
                .branch_id(branch)
                .map(|index| &machine.document().branches[index])
                .ok_or_else(|| error(format!("Unknown branch {branch}")))?;
            if branch
                .actions
                .iter()
                .any(|action| !action.parameters.is_empty())
            {
                return Err(error("branch.knowledge.v1 requires parameterless actions"));
            }
            Ok(())
        }
        _ => Err(error(format!("Unknown standard profile {profile}"))),
    }
}

pub(crate) fn standard(profile: &str) -> bool {
    matches!(profile, "snapshot.v1" | "branch.knowledge.v1")
}
