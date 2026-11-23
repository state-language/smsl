use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic, Machine,
    model::{Branch, Datum, Document},
};

use super::{
    RuntimeState,
    types::unique_map,
    validation::{self, Resolver, error},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BranchMigration {
    #[serde(deserialize_with = "Option::deserialize")]
    pub source: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub state: Option<String>,
    #[serde(deserialize_with = "unique_map")]
    pub facts: BTreeMap<String, Datum>,
    #[serde(deserialize_with = "unique_map")]
    pub data: BTreeMap<String, Datum>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    #[serde(deserialize_with = "unique_map")]
    pub branches: BTreeMap<String, BranchMigration>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    pub base_revision: String,
    pub base_generation: u64,
    pub base_step: u64,
    pub document: Document,
    pub migration: Migration,
}

pub struct ModelEditor {
    change: ChangeSet,
}

impl ModelEditor {
    pub fn new(machine: &Machine, state: &RuntimeState) -> Result<Self, Diagnostic> {
        state.check_machine(machine)?;
        Ok(Self {
            change: ChangeSet {
                base_revision: machine.revision().into(),
                base_generation: state.generation,
                base_step: state.step,
                document: machine.document().clone(),
                migration: Migration::default(),
            },
        })
    }

    pub fn document(&self) -> &Document {
        &self.change.document
    }

    pub fn branch_mut(&mut self, name: &str) -> Result<&mut Branch, Diagnostic> {
        let branch = self
            .change
            .document
            .branches
            .iter_mut()
            .find(|branch| branch.name == name)
            .ok_or_else(|| error("unknown_reference", format!("Unknown branch {name:?}")))?;
        if !branch.mutable {
            return Err(error(
                "immutable_branch",
                format!("Branch {name:?} is structurally immutable"),
            ));
        }
        Ok(branch)
    }

    pub fn add_branch(&mut self, branch: Branch) -> Result<(), Diagnostic> {
        if self
            .change
            .document
            .branches
            .iter()
            .any(|existing| existing.name == branch.name)
        {
            return Err(error(
                "duplicate_name",
                format!("Branch {:?} already exists", branch.name),
            ));
        }
        self.change.document.branches.push(branch);
        Ok(())
    }

    pub fn remove_branch(&mut self, name: &str) -> Result<Branch, Diagnostic> {
        self.branch_mut(name)?;
        let index = self
            .change
            .document
            .branches
            .iter()
            .position(|branch| branch.name == name)
            .expect("checked branch");
        Ok(self.change.document.branches.remove(index))
    }

    pub fn finish(mut self, migration: Migration) -> ChangeSet {
        self.change.migration = migration;
        self.change
    }
}

pub fn evolve(
    machine: &Machine,
    state: &RuntimeState,
    change: &ChangeSet,
) -> Result<(Arc<Machine>, RuntimeState), Diagnostic> {
    state.check_standalone()?;
    evolve_with(machine, state, change, &|_| false)
}

pub(crate) fn evolve_with(
    machine: &Machine,
    state: &RuntimeState,
    change: &ChangeSet,
    resolve: &Resolver<'_>,
) -> Result<(Arc<Machine>, RuntimeState), Diagnostic> {
    state.check_machine(machine)?;
    if change.base_revision != machine.revision()
        || change.base_generation != state.generation
        || change.base_step != state.step
    {
        return Err(error(
            "stale_revision",
            "Change set does not match the current model generation and event step",
        ));
    }
    let candidate = Arc::new(crate::compile(change.document.clone())?);
    let before = machine.document();
    let after = candidate.document();
    for branch in &before.branches {
        if !branch.mutable
            && after.branches.iter().find(|next| next.name == branch.name) != Some(branch)
        {
            return Err(error(
                "immutable_branch",
                format!(
                    "Cannot modify, rename, or remove immutable branch {:?}",
                    branch.name
                ),
            ));
        }
    }
    let affected: BTreeSet<_> = after
        .branches
        .iter()
        .filter(|branch| {
            before.branches.iter().find(|old| old.name == branch.name) != Some(*branch)
        })
        .map(|branch| branch.name.as_str())
        .collect();
    if change
        .migration
        .branches
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != affected
    {
        return Err(error(
            "invalid_migration",
            "Migration must map exactly the new and structurally changed branches",
        ));
    }
    let mut next = RuntimeState::initial(&candidate, state.context.clone())?;
    next.step = state.step;
    next.time = state.time;
    next.generation = state
        .generation
        .checked_add(1)
        .ok_or_else(|| error("invalid_migration", "Model generation overflow"))?;
    let mut sources = BTreeSet::new();
    for branch in &after.branches {
        let Some(mapping) = change.migration.branches.get(&branch.name) else {
            next.branches.insert(
                branch.name.clone(),
                Arc::clone(&state.branches[&branch.name]),
            );
            continue;
        };
        if let Some(source) = &mapping.source {
            if !sources.insert(source) || !state.branches.contains_key(source) {
                return Err(error(
                    "invalid_migration",
                    "Migration sources must identify distinct existing branches",
                ));
            }
            if source != &branch.name && after.branches.iter().any(|branch| branch.name == *source)
            {
                return Err(error(
                    "invalid_migration",
                    "A renamed source branch must not remain as a second branch",
                ));
            }
            let old = before
                .branches
                .iter()
                .find(|branch| branch.name == *source)
                .expect("checked source");
            if !old.mutable {
                return Err(error(
                    "immutable_branch",
                    "Cannot rename or replace an immutable branch",
                ));
            }
        } else if state.branches.contains_key(&branch.name) {
            return Err(error(
                "invalid_migration",
                "An existing branch requires an explicit migration source",
            ));
        }
        let stored: BTreeSet<_> = branch
            .facts
            .iter()
            .filter(|fact| {
                fact.derived.is_none()
                    && !branch
                        .sub_branches
                        .iter()
                        .any(|binding| binding.fact == fact.name)
            })
            .map(|fact| fact.name.as_str())
            .collect();
        if mapping
            .facts
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != stored
        {
            return Err(error(
                "invalid_migration",
                "Migration must supply exactly the stored facts; derived facts are recomputed",
            ));
        }
        let current = crate::language::semantics::FactoredBranch::edit(
            next.branches
                .get_mut(&branch.name)
                .expect("candidate branch"),
        );
        if let Some(name) = &mapping.state {
            current
                .facts
                .insert("state".into(), Datum::String(name.clone()));
        } else {
            current.facts.remove("state");
        }
        current.data = mapping.data.clone();
        current.facts.extend(mapping.facts.clone());
    }
    next.recompute(&candidate)?;
    validation::validate_values(&candidate, &next, resolve)?;
    next.validate_constraints(&candidate)?;
    next.refresh_references();
    Ok((candidate, next))
}
