use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, finite::identity, runtime::RuntimeState};

use super::{Planner, invalid};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchLimits {
    /// Maximum distinct states retained, including the initial state.
    pub max_states: usize,
    /// Maximum number of operator applications in a plan.
    pub max_depth: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchOutcome {
    Found { plan: Vec<String> },
    Unreachable,
    StateLimit,
    DepthLimit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    pub outcome: SearchOutcome,
    pub visited: usize,
    pub expanded: usize,
}

struct Node {
    state: RuntimeState,
    parent: Option<(usize, usize)>,
    depth: usize,
}

impl Planner<'_> {
    /// Breadth-first search minimizes the number of supplied grounded operators.
    pub fn search(
        &self,
        initial: &RuntimeState,
        limits: SearchLimits,
    ) -> Result<SearchResult, Diagnostic> {
        if limits.max_states == 0 {
            return Err(invalid(
                "max_states must include at least the initial state",
            ));
        }
        self.check_state(initial)?;
        let mut nodes = vec![Node {
            state: initial.clone(),
            parent: None,
            depth: 0,
        }];
        let mut seen = HashSet::from([identity(initial)?]);
        let mut expanded = 0;
        let mut depth_limited = false;
        if self.goal_holds(initial)? {
            return Ok(SearchResult {
                outcome: SearchOutcome::Found { plan: Vec::new() },
                visited: 1,
                expanded,
            });
        }
        while expanded < nodes.len() {
            let index = expanded;
            expanded += 1;
            for (operator_index, operator) in self.domain.operators.iter().enumerate() {
                let Some(state) = self.apply(&nodes[index].state, operator)? else {
                    continue;
                };
                let key = identity(&state)?;
                if seen.contains(&key) {
                    continue;
                }
                if nodes[index].depth == limits.max_depth {
                    depth_limited = true;
                    continue;
                }
                if nodes.len() == limits.max_states {
                    return Ok(SearchResult {
                        outcome: SearchOutcome::StateLimit,
                        visited: nodes.len(),
                        expanded,
                    });
                }
                let reached = self.goal_holds(&state)?;
                seen.insert(key);
                nodes.push(Node {
                    state,
                    parent: Some((index, operator_index)),
                    depth: nodes[index].depth + 1,
                });
                if reached {
                    let mut plan = Vec::new();
                    let mut cursor = nodes.len() - 1;
                    while let Some((parent, operator_index)) = nodes[cursor].parent {
                        plan.push(self.domain.operators[operator_index].name.clone());
                        cursor = parent;
                    }
                    plan.reverse();
                    return Ok(SearchResult {
                        outcome: SearchOutcome::Found { plan },
                        visited: nodes.len(),
                        expanded,
                    });
                }
            }
        }
        Ok(SearchResult {
            outcome: if depth_limited {
                SearchOutcome::DepthLimit
            } else {
                SearchOutcome::Unreachable
            },
            visited: nodes.len(),
            expanded,
        })
    }
}
