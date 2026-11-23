use serde::Deserialize;
use serde_json::{Value, json};

use super::{Client, protocol};
use crate::{
    Diagnostic,
    runtime::system::{Invocation, error},
};

impl Client {
    pub fn request(&mut self, source: &str) -> Result<String, Diagnostic> {
        let value = crate::encoding::decode(source, crate::encoding::Encoding::Json)?;
        let request: Request = serde_json::from_value(value)
            .map_err(|cause| error("remote_request", cause.to_string()))?;
        let response = match request {
            Request::Inspect {} => self.inspect(),
            Request::Begin { invocation, now_ms } => self.begin(invocation, now_ms)?,
            Request::Description {
                response,
                nonce,
                now_ms,
            } => json!(self.description(response, &nonce, now_ms)?),
            Request::Opened { response, now_ms } => {
                self.opened(response, now_ms)?;
                Value::Null
            }
            Request::Submit { request_id, now_ms } => json!(self.submit(&request_id, now_ms)?),
            Request::Poll { request_id, now_ms } => json!(self.poll(&request_id, now_ms)?),
            Request::Operation {
                request_id,
                response,
                now_ms,
            } => self.operation(&request_id, response, now_ms)?,
            Request::Cancel { request_id, now_ms } => json!(self.cancel(&request_id, now_ms)?),
            Request::Cancelled {
                request_id,
                response,
                now_ms,
            } => json!(self.cancelled(&request_id, response, now_ms)?),
            Request::Retry { now_ms, message } => {
                self.retry(now_ms, &message)?;
                Value::Null
            }
            Request::CancelRetry {
                request_id,
                now_ms,
                error,
            } => json!(self.cancel_retry(&request_id, now_ms, &error.code, &error.message)?),
            Request::Finish {
                request_id,
                now_ms,
                error,
            } => {
                self.finish(
                    &request_id,
                    now_ms,
                    error.map(|value| (value.code, value.message)),
                )?;
                Value::Null
            }
            Request::Close { now_ms } => json!(self.close(now_ms)?),
            Request::Closed { response } => {
                self.closed(response)?;
                Value::Null
            }
            Request::CloseFailed { now_ms, error } => {
                self.close_failed(now_ms, &error.code, &error.message)?;
                Value::Null
            }
            Request::ErrorResponse { response } => return Err(protocol::remote_error(&response)?),
        };
        Ok(serde_json::to_string(&response).expect("remote response"))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    code: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Inspect {},
    Begin {
        invocation: Invocation,
        now_ms: u64,
    },
    Description {
        response: Value,
        nonce: String,
        now_ms: u64,
    },
    Opened {
        response: Value,
        now_ms: u64,
    },
    Submit {
        request_id: String,
        now_ms: u64,
    },
    Poll {
        request_id: String,
        now_ms: u64,
    },
    Operation {
        request_id: String,
        response: Value,
        now_ms: u64,
    },
    Cancel {
        request_id: String,
        now_ms: u64,
    },
    Cancelled {
        request_id: String,
        response: Value,
        now_ms: u64,
    },
    Retry {
        now_ms: u64,
        message: String,
    },
    CancelRetry {
        request_id: String,
        now_ms: u64,
        error: Failure,
    },
    Finish {
        request_id: String,
        now_ms: u64,
        error: Option<Failure>,
    },
    Close {
        now_ms: u64,
    },
    Closed {
        response: Option<Value>,
    },
    CloseFailed {
        now_ms: u64,
        error: Failure,
    },
    ErrorResponse {
        response: Value,
    },
}
