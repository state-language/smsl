mod lifecycle;
mod model;
mod traversal;

use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, runtime::Step};

use super::{Block, Command, Session, Status, error, session::Progress};

struct Turn<'a> {
    session: &'a mut Session,
    times: &'a BTreeMap<String, u64>,
    remaining: u64,
    active: BTreeSet<String>,
    original_pending: BTreeSet<String>,
    pruned: BTreeSet<String>,
    scopes: BTreeSet<String>,
    commands: Vec<Command>,
    steps: Vec<Step>,
}

impl Session {
    pub(crate) fn tick_inner(
        &mut self,
        times: &BTreeMap<String, u64>,
        budget: u64,
        commands: &mut Vec<Command>,
        steps: &mut Vec<Step>,
    ) -> Result<(), Diagnostic> {
        if budget == 0 || budget > 1_000_000 {
            return Err(error(
                "execution_budget",
                "Tick budget must be between 1 and 1000000",
            ));
        }
        for (instance, time) in times {
            let state = self.world.state(instance).ok_or_else(|| {
                error(
                    "unknown_reference",
                    format!("Unknown clock instance {instance:?}"),
                )
            })?;
            if state.time().is_some_and(|previous| previous > *time)
                || self
                    .times
                    .get(instance)
                    .is_some_and(|previous| previous > time)
            {
                return Err(error("invalid_event", "Execution tick precedes model time"));
            }
        }
        for instance in self.definition.blocks.values().filter_map(Block::instance) {
            if !times.contains_key(instance) {
                return Err(error(
                    "invalid_event",
                    format!("Tick requires a time for instance {instance:?}"),
                ));
            }
        }
        self.times.extend(
            times
                .iter()
                .map(|(instance, time)| (instance.clone(), *time)),
        );
        if self.halted || self.status() == Status::Cancelling || self.status().terminal() {
            return Ok(());
        }
        let checkpoint = self.checkpoint();
        let root = self.definition.root.clone();
        let original_pending = self.pending.keys().cloned().collect();
        let mut turn = Turn {
            session: self,
            times,
            remaining: budget,
            active: BTreeSet::new(),
            original_pending,
            pruned: BTreeSet::new(),
            scopes: BTreeSet::new(),
            commands: Vec::new(),
            steps: Vec::new(),
        };
        turn.visit(&root)?;
        let abandoned: Vec<_> = checkpoint
            .pending
            .keys()
            .filter(|block| !turn.active.contains(*block))
            .cloned()
            .collect();
        let scopes: Vec<_> = checkpoint
            .progress
            .iter()
            .filter(|(id, progress)| {
                progress.entered
                    && (!turn.scopes.contains(*id)
                        || (turn.pruned.contains(*id) && turn.session.progress[*id].entered))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let starts = turn.commands.iter().any(|command| matches!(command, Command::Start { block, .. } if turn.active.contains(block)));
        let preempt = abandoned
            .iter()
            .chain(scopes.iter())
            .any(|id| !turn.pruned.contains(id))
            || ((!abandoned.is_empty() || !scopes.is_empty()) && starts);
        if preempt {
            drop(turn);
            self.rollback(checkpoint);
            self.stopping.extend(scopes);
            self.cancel(&abandoned, commands);
            self.finish_stops(steps)?;
            return Ok(());
        }
        let unstarted: Vec<_> = turn
            .session
            .pending
            .keys()
            .filter(|block| {
                !turn.active.contains(*block) && !checkpoint.pending.contains_key(*block)
            })
            .cloned()
            .collect();
        for block in unstarted {
            let pending = turn.session.pending.remove(&block).expect("pending block");
            turn.session.world.cancel_action(&pending.ticket)?;
            turn.session
                .progress
                .insert(block.clone(), Progress::default());
            turn.commands.retain(
                |command| !matches!(command, Command::Start { block: id, .. } if id == &block),
            );
        }
        turn.session.stopping.extend(scopes);
        turn.session.cancel(&abandoned, &mut turn.commands);
        turn.session.finish_stops(&mut turn.steps)?;
        commands.append(&mut turn.commands);
        steps.append(&mut turn.steps);
        Ok(())
    }
}
