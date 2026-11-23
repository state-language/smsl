use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    Diagnostic,
    application::{ApplicationDefinition, ApplicationLifecycle},
    execution,
    model::Datum,
    runtime::{
        Event, Step, World,
        system::{Basis, LifecyclePage},
    },
};

#[derive(Debug)]
pub struct LifecycleBinding {
    block: String,
    target: ApplicationLifecycle,
    identity: Basis,
    cursor: u64,
}

#[derive(Debug, Serialize)]
pub struct LifecycleProjection {
    pub next: u64,
    pub events: Vec<Step>,
}

impl LifecycleBinding {
    pub fn new(
        application: &ApplicationDefinition,
        world: &World,
        block: &str,
    ) -> Result<Self, Diagnostic> {
        super::validate_world(application, world)?;
        let target = application
            .document()
            .bindings
            .get(block)
            .and_then(|binding| binding.lifecycle.clone())
            .ok_or_else(|| {
                error(
                    "invalid_lifecycle_binding",
                    "Block has no declared lifecycle binding",
                )
            })?;
        let identity = super::world_basis(world, &target.instance)?;
        Ok(Self {
            block: block.into(),
            target,
            identity,
            cursor: 0,
        })
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn apply(
        &mut self,
        world: &mut World,
        page: LifecyclePage,
        time: u64,
    ) -> Result<LifecycleProjection, Diagnostic> {
        let events = self.project(world, &page, time)?;
        let checkpoint = world.checkpoint_execution();
        let mut steps = Vec::with_capacity(events.len());
        for event in events {
            match world.advance(&self.target.instance, &event) {
                Ok(step) => steps.push(step.into_step()),
                Err(cause) => {
                    world.rollback_execution(checkpoint);
                    return Err(cause);
                }
            }
        }
        Ok(self.commit(world, page.next, steps))
    }

    pub fn apply_execution(
        &mut self,
        execution: &mut execution::Session,
        page: LifecyclePage,
        time: u64,
    ) -> Result<LifecycleProjection, Diagnostic> {
        let events = self.project(execution.world(), &page, time)?;
        let steps = execution.observe_lifecycle(&self.target.instance, events)?;
        Ok(self.commit(execution.world(), page.next, steps))
    }

    fn project(
        &self,
        world: &World,
        page: &LifecyclePage,
        time: u64,
    ) -> Result<Vec<Event>, Diagnostic> {
        if page.truncated || page.first > self.cursor {
            return Err(error(
                "lifecycle_history_gap",
                "Lifecycle history was truncated before delivery",
            ));
        }
        if page.cursor != self.cursor || page.next < page.cursor {
            return Err(error(
                "invalid_lifecycle_cursor",
                "Lifecycle page does not continue the binding cursor",
            ));
        }
        let mut previous = None;
        for entry in &page.entries {
            if entry.index < page.cursor
                || entry.index >= page.next
                || previous.is_some_and(|index| entry.index <= index)
                || entry.request_id.trim().is_empty()
                || entry.block.trim().is_empty()
                || entry.basis.revision.trim().is_empty()
            {
                return Err(error(
                    "invalid_lifecycle_page",
                    "Lifecycle entries must be ordered and identify their invocation",
                ));
            }
            previous = Some(entry.index);
        }
        let basis = super::world_basis(world, &self.target.instance)?;
        if basis.revision != self.identity.revision || basis.tick < self.identity.tick {
            return Err(error(
                "stale_lifecycle_binding",
                "Controller identity changed or its history moved backward",
            ));
        }
        let context = world
            .state(&self.target.instance)
            .expect("checked instance")
            .context()
            .clone();
        Ok(page
            .entries
            .iter()
            .filter(|entry| entry.block == self.block)
            .map(|entry| Event {
                name: self.target.event.clone(),
                time,
                context: context.clone(),
                cause: None,
                payload: BTreeMap::from([
                    ("kind".into(), Datum::String(entry.kind.as_str().into())),
                    ("block".into(), Datum::String(entry.block.clone())),
                    ("request_id".into(), Datum::String(entry.request_id.clone())),
                    (
                        "basis_revision".into(),
                        Datum::String(entry.basis.revision.clone()),
                    ),
                    ("basis_tick".into(), Datum::Time(entry.basis.tick)),
                ]),
            })
            .collect())
    }

    fn commit(&mut self, world: &World, next: u64, events: Vec<Step>) -> LifecycleProjection {
        self.cursor = next;
        self.identity =
            super::world_basis(world, &self.target.instance).expect("checked lifecycle instance");
        LifecycleProjection {
            next: self.cursor,
            events,
        }
    }
}

fn error(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message, None)
}
