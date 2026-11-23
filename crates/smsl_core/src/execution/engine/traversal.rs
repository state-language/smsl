use super::super::{
    Block, Command, Mode, Status, error,
    session::{Pending, Progress},
};
use super::Turn;
use crate::{Diagnostic, language::semantics::Evaluation, model::Datum};

impl Turn<'_> {
    pub(super) fn visit(&mut self, id: &str) -> Result<Status, Diagnostic> {
        if self.remaining == 0 {
            return Err(error(
                "execution_budget",
                "Tick exhausted its block visit budget",
            ));
        }
        self.remaining -= 1;
        let block = self.session.definition.blocks[id].clone();
        if let Some(status) = self.session.progress[id].closing {
            if self
                .session
                .subtree(id)
                .iter()
                .any(|id| self.session.pending.contains_key(id))
            {
                self.deactivate_tree(id);
                return Ok(Status::Running);
            }
            let progress = self.session.progress.get_mut(id).expect("declared block");
            progress.closing = None;
            progress.status = status;
            return Ok(status);
        }
        let composite = !block.members().is_empty();
        let result = match block {
            Block::Sequence { members, mode } => self.ordered(id, &members, mode, false)?,
            Block::FirstSuccess { members, mode } => self.ordered(id, &members, mode, true)?,
            Block::Parallel {
                members,
                success_threshold,
                failure_threshold,
            } => self.parallel(&members, success_threshold, failure_threshold)?,
            Block::Scope {
                member,
                enter,
                exit,
                ..
            } => {
                self.scopes.insert(id.into());
                if !self.session.progress[id].entered {
                    if self.session.progress.iter().any(|(other, progress)| {
                        progress.entered && self.session.exclusive(id, other)
                    }) {
                        return Ok(Status::Running);
                    }
                    if let Some(enter) = enter {
                        self.lifecycle(&enter)?;
                    }
                    self.session
                        .progress
                        .get_mut(id)
                        .expect("scope block")
                        .entered = true;
                }
                let status = self.visit(&member)?;
                if status.terminal() {
                    if let Some(exit) = exit {
                        self.lifecycle(&exit)?;
                    }
                    self.session
                        .progress
                        .get_mut(id)
                        .expect("scope block")
                        .entered = false;
                }
                status
            }
            Block::Condition {
                instance,
                branch,
                expression,
            } => {
                let machine = self
                    .session
                    .world
                    .machine(&instance)
                    .expect("validated binding");
                let state = self
                    .session
                    .world
                    .state(&instance)
                    .expect("validated binding");
                let index = machine.branch_id(&branch).expect("validated branch");
                match Evaluation::state(&state.branches, &machine.document().branches[index])
                    .predicate(&expression)?
                {
                    Datum::Bool(true) => Status::Success,
                    Datum::Bool(false) => Status::Failure,
                    Datum::Unknown(_) => Status::Unknown,
                    _ => unreachable!("predicate value"),
                }
            }
            Block::Operation {
                instance,
                operations,
            } => {
                if self.session.progress[id].status.terminal() {
                    self.session.progress[id].status
                } else {
                    self.session.perform_operation(
                        &instance,
                        &operations,
                        self.times,
                        &mut self.steps,
                    )?
                }
            }
            Block::Action {
                instance,
                branch,
                action,
                arguments,
                ..
            } => {
                if self.session.progress[id].status.terminal() {
                    self.session
                        .progress
                        .get_mut(id)
                        .expect("action block")
                        .completion_ready = false;
                    self.session.progress[id].status
                } else if self.session.pending.contains_key(id) {
                    self.active.insert(id.into());
                    Status::Running
                } else if self
                    .session
                    .world
                    .action_applicable(&instance, &branch, &action, &arguments)?
                {
                    let ticket = self
                        .session
                        .world
                        .begin_action(&instance, &branch, &action, arguments)?;
                    self.session.pending.insert(
                        id.into(),
                        Pending {
                            ticket: ticket.clone(),
                            cancelling: false,
                        },
                    );
                    self.active.insert(id.into());
                    self.commands.push(Command::Start {
                        block: id.into(),
                        ticket,
                    });
                    Status::Running
                } else {
                    let machine = self
                        .session
                        .world
                        .machine(&instance)
                        .expect("validated instance");
                    let state = self
                        .session
                        .world
                        .state(&instance)
                        .expect("validated instance");
                    let owner = &machine.document().branches
                        [machine.branch_id(&branch).expect("validated branch")];
                    let declared = owner
                        .actions
                        .iter()
                        .find(|item| item.name == action)
                        .expect("validated action");
                    let evaluation = Evaluation {
                        args: Some(&arguments),
                        ..Evaluation::state(&state.branches, owner)
                    };
                    let unknown = match &declared.precondition {
                        Some(condition) => {
                            matches!(evaluation.predicate(condition)?, Datum::Unknown(_))
                        }
                        None => false,
                    };
                    if unknown {
                        Status::Unknown
                    } else {
                        Status::Failure
                    }
                }
            }
        };
        let result = if composite && result.terminal() {
            self.deactivate_tree(id);
            if self.session.subtree(id).iter().any(|id| {
                self.original_pending.contains(id) && self.session.pending.contains_key(id)
            }) {
                self.session
                    .progress
                    .get_mut(id)
                    .expect("declared block")
                    .closing = Some(result);
                Status::Running
            } else {
                result
            }
        } else {
            result
        };
        self.session
            .progress
            .get_mut(id)
            .expect("declared block")
            .status = result;
        Ok(result)
    }

    fn ordered(
        &mut self,
        id: &str,
        members: &[String],
        mode: Mode,
        choice: bool,
    ) -> Result<Status, Diagnostic> {
        let start = if mode == Mode::Memory {
            self.session.progress[id].cursor
        } else {
            0
        };
        for (index, member) in members.iter().enumerate().skip(start) {
            if mode == Mode::Reactive
                && self.session.progress[member].status.terminal()
                && !self.session.progress[member].completion_ready
            {
                self.reset_tree(member);
            }
            let status = self.visit(member)?;
            let passes = if choice {
                status == Status::Failure
            } else {
                status == Status::Success
            };
            if passes {
                self.session
                    .progress
                    .get_mut(id)
                    .expect("declared block")
                    .cursor = index + 1;
            } else {
                self.session
                    .progress
                    .get_mut(id)
                    .expect("declared block")
                    .cursor = index;
                return Ok(status);
            }
        }
        Ok(if choice {
            Status::Failure
        } else {
            Status::Success
        })
    }

    fn parallel(
        &mut self,
        members: &[String],
        success_threshold: u64,
        failure_threshold: u64,
    ) -> Result<Status, Diagnostic> {
        let mut successes = 0;
        let mut failures = 0;
        let mut running = false;
        for member in members {
            let status = if self.session.progress[member].status.terminal() {
                self.session.progress[member].status
            } else {
                self.visit(member)?
            };
            match status {
                Status::Success => successes += 1,
                Status::Failure => failures += 1,
                Status::Running => running = true,
                _ => {}
            }
            if successes >= success_threshold || failures >= failure_threshold {
                for member in members {
                    self.deactivate_tree(member);
                }
                return Ok(if successes >= success_threshold {
                    Status::Success
                } else {
                    Status::Failure
                });
            }
        }
        Ok(if running {
            Status::Running
        } else {
            Status::Unknown
        })
    }

    fn lifecycle(&mut self, id: &str) -> Result<(), Diagnostic> {
        if self.remaining == 0 {
            return Err(error(
                "execution_budget",
                "Tick exhausted its lifecycle budget",
            ));
        }
        self.remaining -= 1;
        self.session.hook(id, &mut self.steps)
    }

    fn reset_tree(&mut self, id: &str) {
        self.session.progress.insert(id.into(), Progress::default());
        let members = self.session.definition.blocks[id].members().to_vec();
        for member in members {
            self.reset_tree(&member);
        }
    }

    fn deactivate_tree(&mut self, id: &str) {
        self.active.remove(id);
        self.pruned.insert(id.into());
        let members = self.session.definition.blocks[id].members().to_vec();
        for member in members {
            self.deactivate_tree(&member);
        }
    }
}
