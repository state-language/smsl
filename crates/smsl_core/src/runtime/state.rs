use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Diagnostic, Machine,
    language::{
        model::Datum,
        semantics::{self, BranchValues, FactoredBranch},
    },
};

use super::{BranchSnapshot, Context, Event, SNAPSHOT_VERSION, Snapshot, validation};

#[derive(Clone, Debug)]
pub struct RuntimeState {
    pub(crate) definition: Arc<str>,
    revision: Arc<str>,
    pub(crate) context: Context,
    pub(crate) step: u64,
    pub(crate) generation: u64,
    pub(crate) time: Option<u64>,
    pub(crate) branches: BranchValues,
    pub(crate) foreign_references: bool,
}

impl RuntimeState {
    pub fn initial(machine: &Machine, context: Context) -> Result<Self, Diagnostic> {
        validation::context(&context, "invalid_snapshot")?;
        Ok(Self {
            definition: Arc::clone(machine.canonical_source()),
            revision: machine.revision().into(),
            context,
            step: 0,
            generation: 0,
            time: None,
            branches: semantics::initial(machine)?,
            foreign_references: false,
        })
    }

    pub fn from_snapshot(machine: &Machine, snapshot: Snapshot) -> Result<Self, Diagnostic> {
        validation::validate_snapshot(machine, &snapshot)?;
        Ok(Self::from_validated(machine, snapshot))
    }

    pub(crate) fn from_validated(machine: &Machine, snapshot: Snapshot) -> Self {
        let mut state = Self {
            definition: Arc::clone(machine.canonical_source()),
            revision: machine.revision().into(),
            context: snapshot.context,
            step: snapshot.step,
            generation: snapshot.generation,
            time: snapshot.time,
            branches: snapshot
                .branches
                .into_iter()
                .map(|(name, branch)| (name, Arc::new(FactoredBranch::from_snapshot(branch))))
                .collect(),
            foreign_references: false,
        };
        state.refresh_references();
        state
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: SNAPSHOT_VERSION.into(),
            definition: self.definition.to_string(),
            revision: self.revision.to_string(),
            generation: self.generation,
            context: self.context.clone(),
            step: self.step,
            time: self.time,
            branches: self
                .branches
                .iter()
                .map(|(name, branch)| (name.clone(), branch.snapshot().clone()))
                .collect(),
        }
    }

    pub(crate) fn into_snapshot(self) -> Snapshot {
        Snapshot {
            version: SNAPSHOT_VERSION.into(),
            definition: self.definition.to_string(),
            revision: self.revision.to_string(),
            generation: self.generation,
            context: self.context,
            step: self.step,
            time: self.time,
            branches: self
                .branches
                .into_iter()
                .map(|(name, branch)| (name, branch.snapshot().clone()))
                .collect(),
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn revision(&self) -> String {
        self.revision.to_string()
    }
    pub fn step(&self) -> u64 {
        self.step
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn time(&self) -> Option<u64> {
        self.time
    }
    pub fn branch(&self, name: &str) -> Option<&BranchSnapshot> {
        self.branches.get(name).map(|branch| branch.snapshot())
    }
    pub fn branch_names(&self) -> impl Iterator<Item = &str> {
        self.branches.keys().map(String::as_str)
    }

    pub fn reset(&self, machine: &Machine, context: Context) -> Result<Self, Diagnostic> {
        self.check_standalone()?;
        self.check_machine(machine)?;
        validation::context(&context, "invalid_reset")?;
        if context.scope == self.context.scope {
            return Err(
                validation::error("invalid_reset", "Reset requires a new scope")
                    .at_path("/context/scope"),
            );
        }
        Self::initial(machine, context)
    }

    pub(crate) fn check_machine(&self, machine: &Machine) -> Result<(), Diagnostic> {
        if !Arc::ptr_eq(&self.definition, machine.canonical_source())
            && self.definition != *machine.canonical_source()
        {
            return Err(validation::error(
                "invalid_snapshot",
                "Snapshot definition does not match the compiled machine",
            )
            .at_path("/definition"));
        }
        Ok(())
    }

    pub(crate) fn check_standalone(&self) -> Result<(), Diagnostic> {
        if self.foreign_references {
            return Err(validation::error(
                "invalid_snapshot",
                "Cross-scope evidence requires the owning World",
            )
            .at_path("/branches"));
        }
        Ok(())
    }

    pub(crate) fn refresh_references(&mut self) {
        self.foreign_references = self.branches.values().any(|branch| {
            branch.data.values().any(|value| {
            matches!(value, Datum::Event(reference) if reference.scope != self.context.scope)
        })
        });
    }

    pub fn advance(
        &self,
        machine: &Machine,
        event: &Event,
    ) -> Result<super::RuntimeStep, Diagnostic> {
        self.check_standalone()?;
        self.advance_with(machine, event, &|_| false)
    }

    pub fn confirm(
        &self,
        machine: &Machine,
        event: &Event,
        operations: &BTreeMap<String, String>,
    ) -> Result<super::RuntimeStep, Diagnostic> {
        self.check_standalone()?;
        self.confirm_with(machine, event, operations, &|_| false)
    }
}
