use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::runtime::{Event, RuntimeStep, actions::applicable_with, validation::error};
use crate::{Diagnostic, model::Datum};

use super::{World, missing_instance};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTicket {
    pub id: u64,
    pub scope: String,
    pub revision: String,
    pub generation: u64,
    pub branch: String,
    pub action: String,
    #[serde(deserialize_with = "crate::runtime::types::unique_map")]
    pub arguments: BTreeMap<String, Datum>,
}

impl World {
    pub fn action_applicable(
        &self,
        name: &str,
        branch: &str,
        action: &str,
        args: &BTreeMap<String, Datum>,
    ) -> Result<bool, Diagnostic> {
        let scope = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?;
        let current = &self.scopes[scope];
        applicable_with(
            &current.machine,
            &current.state,
            branch,
            action,
            args,
            &|reference| self.resolve(reference).is_some(),
        )
    }

    pub fn begin_action(
        &mut self,
        name: &str,
        branch: &str,
        action: &str,
        args: BTreeMap<String, Datum>,
    ) -> Result<ActionTicket, Diagnostic> {
        if !self.action_applicable(name, branch, action, &args)? {
            return Err(error(
                "action_not_applicable",
                "Action precondition is not true",
            ));
        }
        let scope = self.instances[name].clone();
        let current = &self.scopes[&scope];
        let id = self
            .next_ticket
            .checked_add(1)
            .ok_or_else(|| error("invalid_operation", "Action ticket overflow"))?;
        let ticket = ActionTicket {
            id,
            scope,
            revision: current.machine.revision().into(),
            generation: current.state.generation,
            branch: branch.into(),
            action: action.into(),
            arguments: args,
        };
        self.pending.insert(id, ticket.clone());
        self.next_ticket = id;
        Ok(ticket)
    }

    pub fn complete_action(
        &mut self,
        ticket: &ActionTicket,
        outcome: &Event,
    ) -> Result<RuntimeStep, Diagnostic> {
        let current = self
            .pending
            .get(&ticket.id)
            .ok_or_else(|| error("invalid_operation", "Unknown or completed action ticket"))?;
        if current != ticket {
            return Err(error(
                "invalid_operation",
                "Action ticket does not match the pending action",
            ));
        }
        let scope = &self.scopes[&ticket.scope];
        let branch = &scope.machine.document().branches[scope
            .machine
            .branch_id(&ticket.branch)
            .expect("pending branch")];
        let action = branch
            .actions
            .iter()
            .find(|action| action.name == ticket.action)
            .expect("pending action");
        if !action.outcomes.contains(&outcome.name) {
            return Err(error(
                "invalid_event",
                "Outcome is not declared by the pending action",
            ));
        }
        let result = self.consume(&ticket.scope, outcome, None)?;
        self.pending.remove(&ticket.id);
        Ok(result)
    }

    pub fn cancel_action(&mut self, ticket: &ActionTicket) -> Result<(), Diagnostic> {
        if self.pending.get(&ticket.id) != Some(ticket) {
            return Err(error("invalid_operation", "Unknown action ticket"));
        }
        self.pending.remove(&ticket.id);
        Ok(())
    }

    pub fn pending_action_count(&self) -> usize {
        self.pending.len()
    }

    pub(super) fn quiescent(&self, scope: &str) -> Result<(), Diagnostic> {
        if self.pending.values().any(|ticket| ticket.scope == scope) {
            return Err(error(
                "pending_action",
                "Complete or cancel pending actions before revising, resetting, or retiring this instance",
            ));
        }
        Ok(())
    }
}
