use serde::Serialize;
use serde_json::Value;

use super::{Invocation, Records};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderCommand {
    Create {
        token: u64,
        invocation: Invocation,
    },
    Invoke {
        token: u64,
        invocation: Invocation,
    },
    CurrentBasis {
        token: u64,
        request_id: String,
    },
    ValidateOutputs {
        token: u64,
        invocation: Invocation,
        outputs: Records,
    },
    Cancel {
        token: u64,
        block: String,
        request_id: String,
    },
    Close {
        token: u64,
        block: String,
    },
    Done {
        value: Value,
    },
}

pub(super) enum Next {
    Invoke {
        current_basis: bool,
        validate_outputs: bool,
    },
    Cancel,
}

pub(super) enum Work {
    Create {
        invocation: Invocation,
        next: Next,
    },
    Invoke {
        request_id: String,
        current_basis: bool,
        validate_outputs: bool,
    },
    Basis {
        request_id: String,
        outputs: Records,
        validate_outputs: bool,
        final_check: bool,
    },
    Validate {
        request_id: String,
        outputs: Records,
        current_basis: bool,
    },
    Cancel {
        request_id: String,
    },
    Close {
        remaining: Vec<String>,
        errors: Vec<String>,
    },
}

impl Work {
    pub(super) fn failed_cancellation(&self) -> Option<&str> {
        match self {
            Self::Create {
                invocation,
                next: Next::Cancel,
            } => Some(&invocation.request_id),
            Self::Cancel { request_id } => Some(request_id),
            _ => None,
        }
    }

    pub(super) fn failed_invocation(&self) -> Option<&str> {
        match self {
            Self::Create {
                invocation,
                next: Next::Invoke { .. },
            } => Some(&invocation.request_id),
            Self::Invoke { request_id, .. }
            | Self::Basis { request_id, .. }
            | Self::Validate { request_id, .. } => Some(request_id),
            _ => None,
        }
    }
}
