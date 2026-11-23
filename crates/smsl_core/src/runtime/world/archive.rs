use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic,
    runtime::{
        Context, WORLD_VERSION,
        evolution::ChangeSet,
        types::unique_map,
        validation::{error, key},
    },
};

use super::{Record, World};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeDefinition {
    revision: String,
    context: Context,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    version: String,
    next_ticket: u64,
    #[serde(deserialize_with = "unique_map")]
    definitions: BTreeMap<String, String>,
    scopes: Vec<ScopeDefinition>,
    #[serde(deserialize_with = "unique_map")]
    instances: BTreeMap<String, String>,
    records: Vec<Record>,
}

impl World {
    pub fn to_json(&self) -> Result<String, Diagnostic> {
        if !self.pending.is_empty() {
            return Err(error(
                "pending_action",
                "Saving requires completed or cancelled external actions",
            ));
        }
        let archive = Archive {
            version: WORLD_VERSION.into(),
            next_ticket: self.next_ticket,
            definitions: self
                .definitions
                .iter()
                .map(|(revision, machine)| {
                    (revision.clone(), machine.canonical_source().to_string())
                })
                .collect(),
            scopes: self
                .scopes
                .values()
                .map(|scope| ScopeDefinition {
                    revision: scope.initial_revision.clone(),
                    context: scope.state.context.clone(),
                })
                .collect(),
            instances: self.instances.clone(),
            records: self.records.clone(),
        };
        let source = serde_json::to_string_pretty(&archive)
            .map_err(|cause| error("invalid_world", cause.to_string()))?
            + "\n";
        crate::limits::input(&source, false)?;
        crate::runtime::check_json_profile(&source)?;
        Ok(source)
    }

    pub fn restore(source: &str) -> Result<Self, Diagnostic> {
        crate::limits::input(source, false)?;
        let archive: Archive = serde_json::from_str(source)
            .map_err(|cause| error("invalid_world", format!("Invalid World JSON: {cause}")))?;
        if archive.version != WORLD_VERSION {
            return Err(
                error("invalid_world", "Unsupported World format version").at_path("/version")
            );
        }
        let mut world = Self::new();
        for (revision, source) in archive.definitions {
            let machine = Arc::new(crate::load(&source)?);
            if machine.revision() != revision {
                return Err(error(
                    "invalid_world",
                    "Definition content does not match its revision identity",
                ));
            }
            world.definitions.insert(revision, machine);
        }
        for (index, scope) in archive.scopes.into_iter().enumerate() {
            let machine = Arc::clone(
                world
                    .definitions
                    .get(&scope.revision)
                    .ok_or_else(|| error("invalid_world", "Unknown scope definition revision"))?,
            );
            world
                .add_scope(machine, scope.context)
                .map_err(|error| error.prepend_path(&format!("/scopes/{index}")))?;
        }
        for (index, record) in archive.records.into_iter().enumerate() {
            world
                .replay(record)
                .map_err(|error| error.prepend_path(&format!("/records/{index}")))?;
        }
        let mut active = BTreeSet::new();
        for (name, scope) in &archive.instances {
            if name.is_empty() || !world.scopes.contains_key(scope) || !active.insert(scope) {
                return Err(error(
                    "invalid_world",
                    "Instances require nonempty names and distinct declared scopes",
                )
                .at_path(format!("/instances/{}", key(name))));
            }
        }
        crate::runtime::check_json_profile(source)?;
        world.next_ticket = archive.next_ticket;
        world.instances = archive.instances;
        Ok(world)
    }

    fn replay(&mut self, record: Record) -> Result<(), Diagnostic> {
        match record {
            Record::Event(record) => {
                let scope = self
                    .scopes
                    .get(&record.event.context.scope)
                    .ok_or_else(|| error("invalid_world", "Record uses an undeclared scope"))?;
                if record.revision != scope.machine.revision()
                    || record.reference.scope != record.event.context.scope
                    || scope.state.step.checked_add(1) != Some(record.reference.step)
                {
                    return Err(error(
                        "invalid_world",
                        "Event record revision or logical step is inconsistent",
                    ));
                }
                self.consume(
                    &record.event.context.scope,
                    &record.event,
                    record.operations.as_ref(),
                )?;
            }
            Record::Revision(record) => {
                if !self.scopes.contains_key(&record.scope) {
                    return Err(error(
                        "invalid_world",
                        "Revision record uses an undeclared scope",
                    ));
                }
                let machine = self
                    .definitions
                    .get(&record.revision)
                    .ok_or_else(|| error("invalid_world", "Unknown candidate revision"))?;
                let change = ChangeSet {
                    base_revision: record.base_revision,
                    base_generation: record.base_generation,
                    base_step: record.base_step,
                    document: machine.document().clone(),
                    migration: record.migration,
                };
                self.commit_revision(&record.scope, &change)?;
            }
        }
        Ok(())
    }
}
