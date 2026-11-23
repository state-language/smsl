use serde::Deserialize;
use serde_json::Value;

use super::{ProviderDriver, Records, error};
use crate::Diagnostic;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Invoke {
        block: String,
        request_id: String,
        #[serde(default)]
        inputs: Records,
        #[serde(default)]
        allow_effects: bool,
        #[serde(default)]
        current_basis: bool,
        #[serde(default)]
        validate_outputs: bool,
    },
    Cancel {
        request_id: String,
        block: Option<String>,
    },
    Close {},
    Resume {
        token: u64,
        #[serde(default)]
        value: Value,
        error: Option<String>,
    },
}

impl ProviderDriver {
    pub fn request(&mut self, source: &str) -> Result<String, Diagnostic> {
        self.open()?;
        self.session.request(source)
    }

    pub fn drive(&mut self, source: &str) -> Result<String, Diagnostic> {
        let value = crate::encoding::decode(source, crate::encoding::Encoding::Json)?;
        let request: Request = serde_json::from_value(value)
            .map_err(|cause| error("provider_request", cause.to_string()))?;
        let command = match request {
            Request::Invoke {
                block,
                request_id,
                inputs,
                allow_effects,
                current_basis,
                validate_outputs,
            } => self.invoke(
                &block,
                &request_id,
                inputs,
                allow_effects,
                current_basis,
                validate_outputs,
            )?,
            Request::Cancel { request_id, block } => {
                self.open()?;
                if let Some(block) = block {
                    if self.invocation(&request_id)?.block != block {
                        return Err(error("unknown_request", request_id));
                    }
                }
                self.cancel(&request_id)?
            }
            Request::Close {} => self.close()?,
            Request::Resume {
                token,
                value,
                error,
            } => self.resume(token, value, error)?,
        };
        Ok(serde_json::to_string(&command).expect("provider command"))
    }
}
