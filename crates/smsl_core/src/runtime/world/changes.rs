use std::sync::Arc;

use crate::Diagnostic;

use super::{Record, RevisionRecord, World, missing_instance};
use crate::runtime::{
    RuntimeState,
    evolution::{ChangeSet, evolve_with},
};

impl World {
    pub fn evolve(&mut self, name: &str, change: &ChangeSet) -> Result<RuntimeState, Diagnostic> {
        let scope = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?
            .clone();
        self.commit_revision(&scope, change)
    }

    pub(super) fn commit_revision(
        &mut self,
        scope: &str,
        change: &ChangeSet,
    ) -> Result<RuntimeState, Diagnostic> {
        self.quiescent(scope)?;
        let current = &self.scopes[scope];
        let (machine, state) =
            evolve_with(&current.machine, &current.state, change, &|reference| {
                self.resolve(reference).is_some()
            })?;
        let revision = machine.revision().to_string();
        self.definitions
            .entry(revision.clone())
            .or_insert_with(|| Arc::clone(&machine));
        let current = self.scopes.get_mut(scope).expect("validated scope");
        current.machine = machine;
        current.state = state.clone();
        self.records.push(Record::Revision(RevisionRecord {
            scope: scope.into(),
            base_revision: change.base_revision.clone(),
            base_generation: change.base_generation,
            base_step: change.base_step,
            revision,
            migration: change.migration.clone(),
        }));
        Ok(state)
    }
}
