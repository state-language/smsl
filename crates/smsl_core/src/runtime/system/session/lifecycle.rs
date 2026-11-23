use serde::{Deserialize, Serialize};

use super::Session;
use crate::{
    Diagnostic,
    runtime::system::{Basis, error},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleKind {
    Pending,
    CancellationRequested,
    CancellationRejected,
    Succeeded,
    Failed,
    Cancelled,
}

impl LifecycleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::CancellationRequested => "cancellation_requested",
            Self::CancellationRejected => "cancellation_rejected",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleEntry {
    pub index: u64,
    pub kind: LifecycleKind,
    pub request_id: String,
    pub block: String,
    pub basis: Basis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePage {
    pub cursor: u64,
    pub entries: Vec<LifecycleEntry>,
    pub next: u64,
    pub first: u64,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationPhase {
    Pending,
    Cancelling,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationStatus {
    pub request_id: String,
    pub block: String,
    pub basis: Basis,
    pub phase: InvocationPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleStatus {
    pub basis: Basis,
    pub invocations: Vec<InvocationStatus>,
}

impl Session {
    pub fn lifecycle_status(&self) -> LifecycleStatus {
        LifecycleStatus {
            basis: self.basis.clone(),
            invocations: self
                .pending
                .values()
                .map(|pending| InvocationStatus {
                    request_id: pending.call.request_id.clone(),
                    block: pending.call.block.clone(),
                    basis: pending.call.basis.clone(),
                    phase: if pending.cancelling {
                        InvocationPhase::Cancelling
                    } else {
                        InvocationPhase::Pending
                    },
                })
                .collect(),
        }
    }

    pub fn read_lifecycle(&self, cursor: u64, limit: u64) -> Result<LifecyclePage, Diagnostic> {
        let page = self.read_trace(cursor, limit)?;
        let entries = page
            .entries
            .into_iter()
            .filter_map(|entry| {
                let kind = match entry.kind.as_str() {
                    "call" => LifecycleKind::Pending,
                    "cancellation_requested" => LifecycleKind::CancellationRequested,
                    "cancellation_rejected" => LifecycleKind::CancellationRejected,
                    "output" => LifecycleKind::Succeeded,
                    "failure" => LifecycleKind::Failed,
                    "cancelled" => LifecycleKind::Cancelled,
                    _ => return None,
                };
                Some(LifecycleEntry {
                    index: entry.index,
                    kind,
                    request_id: entry.request_id.expect("invocation trace identity"),
                    block: entry.block.expect("invocation trace block"),
                    basis: entry.invocation_basis.expect("invocation trace basis"),
                })
            })
            .collect();
        Ok(LifecyclePage {
            cursor,
            entries,
            next: page.next,
            first: page.first,
            truncated: page.truncated,
        })
    }

    pub(in crate::runtime::system) fn request_cancellation(
        &mut self,
        request_id: &str,
    ) -> Result<(), Diagnostic> {
        let pending = self
            .pending
            .get_mut(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        if pending.cancelling {
            return Err(error(
                "cancellation_pending",
                "Cancellation is already awaiting acknowledgement",
            ));
        }
        pending.cancelling = true;
        let call = pending.call.clone();
        self.record_invocation("cancellation_requested", &call, None);
        Ok(())
    }

    pub(in crate::runtime::system) fn reject_cancellation(&mut self, request_id: &str) {
        let Some(pending) = self.pending.get_mut(request_id) else {
            return;
        };
        if pending.cancelling {
            pending.cancelling = false;
            let call = pending.call.clone();
            self.record_invocation("cancellation_rejected", &call, None);
        }
    }
}
