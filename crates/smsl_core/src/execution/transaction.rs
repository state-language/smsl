use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
};

use super::{
    Session,
    session::{Pending, Progress},
};
use crate::runtime::ExecutionCheckpoint;

pub(super) struct Checkpoint {
    world: ExecutionCheckpoint,
    pub(super) progress: BTreeMap<String, Progress>,
    pub(super) pending: BTreeMap<String, Pending>,
    halted: bool,
    stopping: BTreeSet<String>,
    times: BTreeMap<String, u64>,
}

impl Session {
    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            world: self.world.checkpoint_execution(),
            progress: self.progress.clone(),
            pending: self.pending.clone(),
            halted: self.halted,
            stopping: self.stopping.clone(),
            times: self.times.clone(),
        }
    }

    pub(super) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.world.rollback_execution(checkpoint.world);
        self.progress = checkpoint.progress;
        self.pending = checkpoint.pending;
        self.halted = checkpoint.halted;
        self.stopping = checkpoint.stopping;
        self.times = checkpoint.times;
    }
}

pub(super) struct Transaction<'a> {
    session: &'a mut Session,
    checkpoint: Option<Checkpoint>,
}

impl<'a> Transaction<'a> {
    pub(super) fn new(session: &'a mut Session) -> Self {
        let checkpoint = Some(session.checkpoint());
        Self {
            session,
            checkpoint,
        }
    }

    pub(super) fn commit(mut self) {
        self.checkpoint.take();
    }
}

impl Deref for Transaction<'_> {
    type Target = Session;
    fn deref(&self) -> &Session {
        self.session
    }
}

impl DerefMut for Transaction<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        self.session
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if let Some(checkpoint) = self.checkpoint.take() {
            self.session.rollback(checkpoint);
        }
    }
}
