use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    encoding::unique_map,
    model::{Datum, Expr},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub version: u64,
    pub id: String,
    pub root: String,
    #[serde(deserialize_with = "unique_map")]
    pub blocks: BTreeMap<String, Block>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Memory,
    Reactive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mechanism", deny_unknown_fields)]
pub enum Block {
    #[serde(rename = "control.sequence")]
    Sequence { members: Vec<String>, mode: Mode },
    #[serde(rename = "control.first_success")]
    FirstSuccess { members: Vec<String>, mode: Mode },
    #[serde(rename = "control.parallel")]
    Parallel {
        members: Vec<String>,
        success_threshold: u64,
        failure_threshold: u64,
    },
    #[serde(rename = "control.scope")]
    Scope {
        member: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        enter: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit: Option<String>,
        retain: bool,
    },
    #[serde(rename = "control.condition")]
    Condition {
        instance: String,
        branch: String,
        expression: Expr,
    },
    #[serde(rename = "control.operation")]
    Operation {
        instance: String,
        #[serde(deserialize_with = "unique_map")]
        operations: BTreeMap<String, String>,
    },
    #[serde(rename = "control.action")]
    Action {
        instance: String,
        branch: String,
        action: String,
        #[serde(deserialize_with = "unique_map")]
        arguments: BTreeMap<String, Datum>,
        success_outcomes: Vec<String>,
    },
}

impl Block {
    pub(crate) fn members(&self) -> Vec<String> {
        match self {
            Self::Sequence { members, .. }
            | Self::FirstSuccess { members, .. }
            | Self::Parallel { members, .. } => members.clone(),
            Self::Scope {
                member,
                enter,
                exit,
                ..
            } => enter
                .iter()
                .chain(std::iter::once(member))
                .chain(exit.iter())
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn instance(&self) -> Option<&str> {
        match self {
            Self::Condition { instance, .. }
            | Self::Operation { instance, .. }
            | Self::Action { instance, .. } => Some(instance),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Idle,
    Running,
    Success,
    Failure,
    Unknown,
    Cancelling,
}

impl Status {
    pub(crate) fn terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failure)
    }
}
