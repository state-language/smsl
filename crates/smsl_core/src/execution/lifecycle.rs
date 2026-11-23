use crate::{
    Diagnostic,
    runtime::{Event, Step},
};

use super::{Input, Session, transaction::Transaction};

impl Session {
    pub(crate) fn observe_lifecycle(
        &mut self,
        instance: &str,
        events: Vec<Event>,
    ) -> Result<Vec<Step>, Diagnostic> {
        let mut transaction = Transaction::new(self);
        let mut inputs = Vec::with_capacity(events.len());
        let mut steps = Vec::with_capacity(events.len());
        for event in events {
            let input = Input::Observe {
                instance: instance.into(),
                event,
            };
            steps.extend(transaction.apply_inner(&input)?.steps);
            inputs.push(input);
        }
        transaction.records.extend(inputs);
        transaction.commit();
        Ok(steps)
    }
}
