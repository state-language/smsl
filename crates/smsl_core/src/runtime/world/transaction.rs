use std::collections::BTreeMap;

use super::{ActionTicket, Record, Scope, World};

pub(crate) struct ExecutionCheckpoint {
    scopes: BTreeMap<String, Scope>,
    pending: BTreeMap<u64, ActionTicket>,
    records: usize,
    next_ticket: u64,
}

impl World {
    // Execution inputs change values, tickets and append evidence, never model structure.
    pub(crate) fn checkpoint_execution(&self) -> ExecutionCheckpoint {
        ExecutionCheckpoint {
            scopes: self.scopes.clone(),
            pending: self.pending.clone(),
            records: self.records.len(),
            next_ticket: self.next_ticket,
        }
    }

    pub(crate) fn rollback_execution(&mut self, checkpoint: ExecutionCheckpoint) {
        for record in self.records.drain(checkpoint.records..) {
            if let Record::Event(record) = record {
                self.references
                    .remove(&(record.reference.scope, record.reference.step));
            }
        }
        self.scopes = checkpoint.scopes;
        self.pending = checkpoint.pending;
        self.next_ticket = checkpoint.next_ticket;
    }
}
