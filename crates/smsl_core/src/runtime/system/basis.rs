use super::Basis;
use crate::runtime::RuntimeState;

impl Basis {
    pub fn from_state(state: &RuntimeState) -> Self {
        Self {
            revision: serde_json::to_string(&(
                state.context(),
                state.revision(),
                state.generation(),
            ))
            .expect("runtime identity"),
            tick: state.step(),
        }
    }
}
