use serde::Deserialize;
use serde_json::{Value, json};

use super::Session;
use crate::{
    Diagnostic,
    runtime::system::{Basis, Record, Records, error},
};

impl Session {
    pub fn request(&mut self, source: &str) -> Result<String, Diagnostic> {
        let value = crate::encoding::decode(source, crate::encoding::Encoding::Json)?;
        let request: Request = serde_json::from_value(value)
            .map_err(|cause| error("system_request", cause.to_string()))?;
        let value = match request {
            Request::SetInput { endpoint, record } => {
                self.set_input(&endpoint, record)?;
                Value::Null
            }
            Request::Prepare {
                block,
                request_id,
                inputs,
                allow_effects,
            } => json!(self.prepare(&block, &request_id, inputs, allow_effects)?),
            Request::Complete {
                request_id,
                outputs,
                current_basis,
            } => json!(match current_basis {
                Some(basis) => self.complete_checked(&request_id, outputs, &basis)?,
                None => self.complete(&request_id, outputs)?,
            }),
            Request::FailIfPending {
                request_id,
                message,
            } => json!(self.fail_if_pending(&request_id, &message)),
            Request::CheckCurrentBasis {
                request_id,
                current_basis,
            } => {
                self.check_current_basis(&request_id, &current_basis)?;
                Value::Null
            }
            Request::Fail {
                request_id,
                message,
            } => {
                self.fail(&request_id, &message)?;
                Value::Null
            }
            Request::Cancel {
                request_id,
                acknowledged,
            } => {
                match acknowledged {
                    Some(acknowledged) => self.cancel_checked(&request_id, acknowledged)?,
                    None => self.cancel(&request_id)?,
                }
                Value::Null
            }
            Request::AssertIdle {} => {
                self.assert_idle()?;
                Value::Null
            }
            Request::Advance { basis } => {
                self.advance(basis)?;
                Value::Null
            }
            Request::Inspect {} => self.inspect(),
            Request::Status {} => self.status(),
            Request::LifecycleStatus {} => json!(self.lifecycle_status()),
            Request::ReadLifecycle { cursor, limit } => json!(self.read_lifecycle(cursor, limit)?),
            Request::ReadTrace { cursor, limit } => json!(self.read_trace(cursor, limit)?),
            Request::SetTraceRetention { limit } => {
                let limit = if limit.is_null() {
                    None
                } else {
                    Some(limit.as_u64().ok_or_else(|| {
                        error(
                            "invalid_trace_retention",
                            "Trace retention must be a positive integer or null",
                        )
                    })?)
                };
                self.set_trace_retention(limit)?;
                Value::Null
            }
        };
        Ok(serde_json::to_string(&value).expect("session response"))
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    SetInput {
        endpoint: String,
        record: Record,
    },
    Prepare {
        block: String,
        request_id: String,
        #[serde(default)]
        inputs: Records,
        #[serde(default)]
        allow_effects: bool,
    },
    Complete {
        request_id: String,
        outputs: Records,
        current_basis: Option<Basis>,
    },
    FailIfPending {
        request_id: String,
        message: String,
    },
    CheckCurrentBasis {
        request_id: String,
        current_basis: Basis,
    },
    Fail {
        request_id: String,
        message: String,
    },
    Cancel {
        request_id: String,
        #[serde(default, deserialize_with = "acknowledgment")]
        acknowledged: Option<bool>,
    },
    AssertIdle {},
    Advance {
        basis: Basis,
    },
    Inspect {},
    Status {},
    LifecycleStatus {},
    ReadLifecycle {
        #[serde(default)]
        cursor: u64,
        #[serde(default = "trace_limit")]
        limit: u64,
    },
    ReadTrace {
        #[serde(default)]
        cursor: u64,
        #[serde(default = "trace_limit")]
        limit: u64,
    },
    SetTraceRetention {
        limit: Value,
    },
}

fn trace_limit() -> u64 {
    100
}

fn acknowledgment<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}
