use serde_json::Value;

use super::{Basis, Diagnostic, ProviderCommand, ProviderDriver, Records, Work, error};

impl ProviderDriver {
    pub(super) fn finish(
        &mut self,
        work: Work,
        value: Value,
        failure: Option<String>,
    ) -> Result<ProviderCommand, Diagnostic> {
        if let Work::Close {
            remaining,
            mut errors,
        } = work
        {
            if let Some(message) = failure {
                errors.push(message);
            }
            return self.cleanup(remaining, errors);
        }
        if let Some(message) = failure {
            return Err(error("provider_error", message));
        }
        match work {
            Work::Create { invocation, next } => {
                self.instances.push(invocation.block.clone());
                self.invocation(&invocation.request_id)?;
                self.dispatch(invocation, next)
            }
            Work::Invoke {
                request_id,
                current_basis,
                validate_outputs,
            } => {
                self.invocation(&request_id)?;
                let outputs: Records = serde_json::from_value(value)
                    .map_err(|cause| error("invalid_output", cause.to_string()))?;
                if current_basis {
                    self.basis(request_id, outputs, validate_outputs, false)
                } else {
                    self.validate(request_id, outputs, validate_outputs, false)
                }
            }
            Work::Basis {
                request_id,
                outputs,
                validate_outputs,
                final_check,
            } => {
                let basis: Basis = serde_json::from_value(value)
                    .map_err(|cause| error("invalid_basis", cause.to_string()))?;
                self.session.check_current_basis(&request_id, &basis)?;
                if final_check {
                    self.complete(&request_id, outputs)
                } else {
                    self.validate(request_id, outputs, validate_outputs, true)
                }
            }
            Work::Validate {
                request_id,
                outputs,
                current_basis,
            } => {
                if current_basis {
                    self.basis(request_id, outputs, false, true)
                } else {
                    self.complete(&request_id, outputs)
                }
            }
            Work::Cancel { request_id } => {
                let acknowledged = value.as_bool().ok_or_else(|| {
                    error(
                        "system_request",
                        "Cancellation acknowledgement must be boolean",
                    )
                })?;
                self.session.cancel_checked(&request_id, acknowledged)?;
                Ok(ProviderCommand::Done { value: Value::Null })
            }
            Work::Close { .. } => unreachable!(),
        }
    }

    fn basis(
        &mut self,
        request_id: String,
        outputs: Records,
        validate_outputs: bool,
        final_check: bool,
    ) -> Result<ProviderCommand, Diagnostic> {
        let token = self.enqueue(Work::Basis {
            request_id: request_id.clone(),
            outputs,
            validate_outputs,
            final_check,
        })?;
        Ok(ProviderCommand::CurrentBasis { token, request_id })
    }

    fn validate(
        &mut self,
        request_id: String,
        outputs: Records,
        validate_outputs: bool,
        current_basis: bool,
    ) -> Result<ProviderCommand, Diagnostic> {
        if validate_outputs {
            let invocation = self.invocation(&request_id)?;
            let token = self.enqueue(Work::Validate {
                request_id,
                outputs: outputs.clone(),
                current_basis,
            })?;
            Ok(ProviderCommand::ValidateOutputs {
                token,
                invocation,
                outputs,
            })
        } else if current_basis {
            self.basis(request_id, outputs, false, true)
        } else {
            self.complete(&request_id, outputs)
        }
    }

    fn complete(
        &mut self,
        request_id: &str,
        outputs: Records,
    ) -> Result<ProviderCommand, Diagnostic> {
        let outputs = self.session.complete(request_id, outputs)?;
        Ok(ProviderCommand::Done {
            value: serde_json::to_value(outputs).expect("provider outputs"),
        })
    }
}
