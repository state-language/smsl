use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic,
    encoding::unique_map,
    runtime::{ActionTicket, Event, Step, World},
};

use super::{Block, Definition, Status, error, transaction::Transaction, validation};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    Tick {
        #[serde(deserialize_with = "unique_map")]
        times: BTreeMap<String, u64>,
        budget: u64,
    },
    Observe {
        instance: String,
        event: Event,
    },
    Dispatch {
        instance: String,
        event: Event,
        budget: u64,
    },
    Complete {
        block: String,
        ticket: u64,
        event: Event,
    },
    Halt {},
    AcknowledgeCancel {
        block: String,
        ticket: u64,
    },
    Reset {},
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Start { block: String, ticket: ActionTicket },
    Cancel { block: String, ticket: ActionTicket },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub status: Status,
    #[serde(deserialize_with = "unique_map")]
    pub states: BTreeMap<String, Status>,
    pub commands: Vec<Command>,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Progress {
    pub status: Status,
    pub cursor: usize,
    pub entered: bool,
    pub closing: Option<Status>,
    pub completion_ready: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub ticket: ActionTicket,
    pub cancelling: bool,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub(crate) definition: Definition,
    pub(crate) world: World,
    pub(crate) initial_world: serde_json::Value,
    pub(crate) records: Vec<Input>,
    pub(crate) progress: BTreeMap<String, Progress>,
    pub(crate) pending: BTreeMap<String, Pending>,
    pub(crate) halted: bool,
    pub(crate) stopping: BTreeSet<String>,
    pub(crate) times: BTreeMap<String, u64>,
}

impl Session {
    pub fn new(definition: Definition, world: World) -> Result<Self, Diagnostic> {
        validation::validate(&definition)?;
        validation::bindings(&definition, &world)?;
        let initial_world =
            crate::encoding::decode(&world.to_json()?, crate::encoding::Encoding::Json)?;
        let progress = definition
            .blocks
            .keys()
            .map(|id| (id.clone(), Progress::default()))
            .collect();
        Ok(Self {
            definition,
            world,
            initial_world,
            records: Vec::new(),
            progress,
            pending: BTreeMap::new(),
            halted: false,
            stopping: BTreeSet::new(),
            times: BTreeMap::new(),
        })
    }

    pub fn definition(&self) -> &Definition {
        &self.definition
    }
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn status(&self) -> Status {
        if self.pending.values().any(|pending| pending.cancelling) {
            Status::Cancelling
        } else if self.halted {
            Status::Idle
        } else {
            self.progress[&self.definition.root].status
        }
    }

    pub fn states(&self) -> BTreeMap<String, Status> {
        self.progress
            .iter()
            .map(|(id, state)| (id.clone(), state.status))
            .collect()
    }

    pub fn apply(&mut self, input: Input) -> Result<Report, Diagnostic> {
        let mut transaction = Transaction::new(self);
        let report = transaction.apply_inner(&input)?;
        transaction.records.push(input);
        transaction.commit();
        Ok(report)
    }

    pub(super) fn apply_inner(&mut self, input: &Input) -> Result<Report, Diagnostic> {
        let mut commands = Vec::new();
        let mut steps = Vec::new();
        match input {
            Input::Tick { times, budget } => {
                self.tick_inner(times, *budget, &mut commands, &mut steps)?
            }
            Input::Observe { instance, event } => {
                self.accept_time(instance, event.time)?;
                steps.push(self.world.advance(instance, event)?.into_step());
            }
            Input::Dispatch {
                instance,
                event,
                budget,
            } => {
                if *budget == 0 || *budget > 1_000_000 {
                    return Err(error(
                        "execution_budget",
                        "Dispatch budget must be between 1 and 1000000",
                    ));
                }
                self.accept_time(instance, event.time)?;
                let mut remaining = *budget;
                let mut queue = VecDeque::from([event.clone()]);
                while let Some(event) = queue.pop_front() {
                    if remaining == 0 {
                        return Err(error(
                            "execution_budget",
                            "Dispatch exhausted its event budget",
                        ));
                    }
                    remaining -= 1;
                    let step = self.world.advance(instance, &event)?.into_step();
                    queue.extend(step.derived_events.iter().cloned());
                    steps.push(step);
                }
            }
            Input::Complete {
                block,
                ticket,
                event,
            } => {
                let pending = self
                    .pending
                    .get(block)
                    .ok_or_else(|| error("invalid_execution", "Block has no pending action"))?;
                if pending.ticket.id != *ticket {
                    return Err(error(
                        "invalid_execution",
                        "Completion does not match the pending action ticket",
                    ));
                }
                let ticket = pending.ticket.clone();
                let Block::Action {
                    instance,
                    success_outcomes,
                    ..
                } = &self.definition.blocks[block]
                else {
                    unreachable!("pending action block")
                };
                let instance = instance.clone();
                let status = if success_outcomes.contains(&event.name) {
                    Status::Success
                } else {
                    Status::Failure
                };
                self.accept_time(&instance, event.time)?;
                let step = self.world.complete_action(&ticket, event)?;
                self.pending.remove(block);
                let progress = self.progress.get_mut(block).expect("declared block");
                progress.status = status;
                progress.completion_ready = true;
                steps.push(step.into_step());
                self.finish_stops(&mut steps)?;
            }
            Input::Halt {} => {
                self.halted = true;
                let pending: Vec<_> = self.pending.keys().cloned().collect();
                self.cancel(&pending, &mut commands);
                self.stopping.extend(
                    self.progress
                        .iter()
                        .filter(|(_, state)| state.entered)
                        .map(|(id, _)| id.clone()),
                );
                self.finish_stops(&mut steps)?;
            }
            Input::AcknowledgeCancel { block, ticket } => {
                let pending = self
                    .pending
                    .get(block)
                    .ok_or_else(|| error("invalid_execution", "Block has no pending action"))?;
                if !pending.cancelling || pending.ticket.id != *ticket {
                    return Err(error(
                        "invalid_execution",
                        "Cancellation acknowledgment does not match a requested stop",
                    ));
                }
                self.world.cancel_action(&pending.ticket)?;
                self.pending.remove(block);
                self.progress.insert(block.clone(), Progress::default());
                self.finish_stops(&mut steps)?;
            }
            Input::Reset {} => {
                if !self.pending.is_empty() || self.progress.values().any(|state| state.entered) {
                    return Err(error(
                        "pending_action",
                        "Stop or complete actions before resetting execution",
                    ));
                }
                self.progress
                    .values_mut()
                    .for_each(|state| *state = Progress::default());
                self.halted = false;
            }
        }
        Ok(Report {
            status: self.status(),
            states: self.states(),
            commands,
            steps,
        })
    }

    pub fn tick(
        &mut self,
        times: BTreeMap<String, u64>,
        budget: u64,
    ) -> Result<Report, Diagnostic> {
        self.apply(Input::Tick { times, budget })
    }

    pub fn observe(
        &mut self,
        instance: impl Into<String>,
        event: Event,
    ) -> Result<Report, Diagnostic> {
        self.apply(Input::Observe {
            instance: instance.into(),
            event,
        })
    }

    pub fn complete(
        &mut self,
        block: impl Into<String>,
        ticket: u64,
        event: Event,
    ) -> Result<Report, Diagnostic> {
        self.apply(Input::Complete {
            block: block.into(),
            ticket,
            event,
        })
    }

    pub fn dispatch(
        &mut self,
        instance: impl Into<String>,
        event: Event,
        budget: u64,
    ) -> Result<Report, Diagnostic> {
        self.apply(Input::Dispatch {
            instance: instance.into(),
            event,
            budget,
        })
    }

    pub fn halt(&mut self) -> Result<Report, Diagnostic> {
        self.apply(Input::Halt {})
    }

    pub fn acknowledge_cancel(
        &mut self,
        block: impl Into<String>,
        ticket: u64,
    ) -> Result<Report, Diagnostic> {
        self.apply(Input::AcknowledgeCancel {
            block: block.into(),
            ticket,
        })
    }

    pub fn reset(&mut self) -> Result<Report, Diagnostic> {
        self.apply(Input::Reset {})
    }

    pub(crate) fn cancel(&mut self, blocks: &[String], commands: &mut Vec<Command>) {
        for block in blocks {
            let pending = self.pending.get_mut(block).expect("pending block");
            if !pending.cancelling {
                pending.cancelling = true;
                self.progress.get_mut(block).expect("declared block").status = Status::Cancelling;
                commands.push(Command::Cancel {
                    block: block.clone(),
                    ticket: pending.ticket.clone(),
                });
            }
        }
    }

    fn accept_time(&mut self, instance: &str, time: u64) -> Result<(), Diagnostic> {
        if self
            .times
            .get(instance)
            .is_some_and(|previous| *previous > time)
        {
            return Err(error("invalid_event", "Event precedes the execution clock"));
        }
        self.times.insert(instance.into(), time);
        Ok(())
    }
}
