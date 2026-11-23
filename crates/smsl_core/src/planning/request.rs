use serde::Deserialize;

use super::{Goal, Operator, Planner, SearchLimits, invalid};
use crate::{
    Diagnostic, Machine,
    encoding::{self, Encoding},
    runtime::RuntimeState,
};

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Analyze {
        #[serde(default)]
        operators: Option<Vec<Operator>>,
    },
    Explore {
        #[serde(default)]
        operators: Option<Vec<Operator>>,
        #[serde(default = "default_max_states")]
        max_states: usize,
    },
    Search {
        goal: Goal,
        #[serde(default)]
        operators: Option<Vec<Operator>>,
        limits: SearchLimits,
    },
    Enabled {
        goal: Goal,
        #[serde(default)]
        operators: Option<Vec<Operator>>,
    },
    Validate {
        goal: Goal,
        #[serde(default)]
        operators: Option<Vec<Operator>>,
        plan: Vec<String>,
    },
}

pub fn request(
    machine: &Machine,
    state: &RuntimeState,
    source: &str,
) -> Result<String, Diagnostic> {
    let request: Request = serde_json::from_value(encoding::decode(source, Encoding::Json)?)
        .map_err(|cause| invalid(cause.to_string()))?;
    let value = match request {
        Request::Analyze { operators } => {
            serde_json::to_value(crate::exploration::analyze(machine, state, operators)?)
        }
        Request::Explore {
            operators,
            max_states,
        } => serde_json::to_value(crate::exploration::explore(
            machine,
            state,
            crate::exploration::Options {
                operators,
                max_states,
            },
        )?),
        Request::Search {
            goal,
            operators,
            limits,
        } => serde_json::to_value(planner(machine, goal, operators)?.search(state, limits)?),
        Request::Enabled { goal, operators } => {
            serde_json::to_value(planner(machine, goal, operators)?.enabled_operations(state)?)
        }
        Request::Validate {
            goal,
            operators,
            plan,
        } => serde_json::to_value(
            planner(machine, goal, operators)?
                .validate_plan(state, &plan)?
                .snapshot(),
        ),
    }
    .expect("planning response serialization");
    Ok(value.to_string())
}

fn planner(
    machine: &Machine,
    goal: Goal,
    operators: Option<Vec<Operator>>,
) -> Result<Planner<'_>, Diagnostic> {
    match operators {
        Some(operators) => Planner::new(machine, goal, operators),
        None => Planner::local(machine, goal),
    }
}

fn default_max_states() -> usize {
    crate::exploration::Options::default().max_states
}
