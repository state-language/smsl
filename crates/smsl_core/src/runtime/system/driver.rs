mod completion;
mod model;
mod protocol;

use std::collections::BTreeMap;

use serde_json::Value;

use super::{
    Basis, Invocation, ProviderCatalog, ProviderDescription, Records, Session, SystemDefinition,
    error,
};
use crate::Diagnostic;

pub use model::ProviderCommand;
use model::{Next, Work};

pub struct ProviderDriver {
    session: Session,
    instances: Vec<String>,
    work: BTreeMap<u64, Work>,
    next_token: u64,
    closing: bool,
    closed: bool,
}

impl ProviderDriver {
    pub fn new(
        definition: SystemDefinition,
        basis: Basis,
        providers: Vec<ProviderDescription>,
    ) -> Result<Self, Diagnostic> {
        let session = Session::new(definition, basis)?;
        ProviderCatalog::new(providers)?.validate_bindings(session.definition())?;
        Ok(Self {
            session,
            instances: Vec::new(),
            work: BTreeMap::new(),
            next_token: 0,
            closing: false,
            closed: false,
        })
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    fn open(&self) -> Result<(), Diagnostic> {
        if self.closing || self.closed {
            return Err(error("closed_resource", "System session is closed"));
        }
        Ok(())
    }

    fn enqueue(&mut self, work: Work) -> Result<u64, Diagnostic> {
        let token = self.next_token;
        self.next_token = token
            .checked_add(1)
            .ok_or_else(|| error("resource_limit", "Provider command identities exhausted"))?;
        self.work.insert(token, work);
        Ok(token)
    }

    fn invocation(&self, request_id: &str) -> Result<Invocation, Diagnostic> {
        self.session
            .pending(request_id)
            .cloned()
            .ok_or_else(|| error("unknown_request", request_id))
    }

    pub fn invoke(
        &mut self,
        block: &str,
        request_id: &str,
        inputs: Records,
        allow_effects: bool,
        current_basis: bool,
        validate_outputs: bool,
    ) -> Result<ProviderCommand, Diagnostic> {
        self.open()?;
        let call = self
            .session
            .prepare(block, request_id, inputs, allow_effects)?;
        let result = self.provider(
            call,
            Next::Invoke {
                current_basis,
                validate_outputs,
            },
        );
        if let Err(cause) = &result {
            self.session.fail_if_pending(request_id, &cause.to_string());
        }
        result
    }

    pub fn cancel(&mut self, request_id: &str) -> Result<ProviderCommand, Diagnostic> {
        self.open()?;
        let call = self.invocation(request_id)?;
        self.session.request_cancellation(request_id)?;
        let result = self.provider(call, Next::Cancel);
        if result.is_err() {
            self.session.reject_cancellation(request_id);
        }
        result
    }

    fn provider(
        &mut self,
        invocation: Invocation,
        next: Next,
    ) -> Result<ProviderCommand, Diagnostic> {
        if self.instances.contains(&invocation.block) {
            return self.dispatch(invocation, next);
        }
        if self.work.values().any(|work| {
            matches!(work,
            Work::Create { invocation: pending, .. } if pending.block == invocation.block)
        }) {
            return Err(error("provider_busy", "Provider creation is in progress"));
        }
        let token = self.enqueue(Work::Create {
            invocation: invocation.clone(),
            next,
        })?;
        Ok(ProviderCommand::Create { token, invocation })
    }

    fn dispatch(
        &mut self,
        invocation: Invocation,
        next: Next,
    ) -> Result<ProviderCommand, Diagnostic> {
        match next {
            Next::Invoke {
                current_basis,
                validate_outputs,
            } => {
                let token = self.enqueue(Work::Invoke {
                    request_id: invocation.request_id.clone(),
                    current_basis,
                    validate_outputs,
                })?;
                Ok(ProviderCommand::Invoke { token, invocation })
            }
            Next::Cancel => {
                let token = self.enqueue(Work::Cancel {
                    request_id: invocation.request_id.clone(),
                })?;
                Ok(ProviderCommand::Cancel {
                    token,
                    block: invocation.block,
                    request_id: invocation.request_id,
                })
            }
        }
    }

    pub fn resume(
        &mut self,
        token: u64,
        value: Value,
        failure: Option<String>,
    ) -> Result<ProviderCommand, Diagnostic> {
        if self.closed {
            return Err(error("closed_resource", "System session is closed"));
        }
        let work = self
            .work
            .remove(&token)
            .ok_or_else(|| error("unknown_command", "Provider command is not outstanding"))?;
        let request_id = work.failed_invocation().map(str::to_owned);
        let cancellation = work.failed_cancellation().map(str::to_owned);
        let message = failure.clone();
        let result = self.finish(work, value, failure);
        if let (Err(_), Some(request_id)) = (&result, cancellation) {
            self.session.reject_cancellation(&request_id);
        }
        if let (Err(cause), Some(request_id)) = (&result, request_id) {
            self.session.fail_if_pending(
                &request_id,
                message.as_deref().unwrap_or(&cause.to_string()),
            );
        }
        result
    }

    pub fn close(&mut self) -> Result<ProviderCommand, Diagnostic> {
        if self.closed {
            return Ok(ProviderCommand::Done { value: Value::Null });
        }
        self.open()?;
        self.session.assert_idle()?;
        if self
            .work
            .values()
            .any(|work| matches!(work, Work::Create { .. }))
        {
            return Err(error("provider_busy", "Provider creation is in progress"));
        }
        self.closing = true;
        self.work.clear();
        let remaining = std::mem::take(&mut self.instances);
        self.cleanup(remaining, Vec::new())
    }

    fn cleanup(
        &mut self,
        mut remaining: Vec<String>,
        errors: Vec<String>,
    ) -> Result<ProviderCommand, Diagnostic> {
        if let Some(block) = remaining.pop() {
            let token = self.enqueue(Work::Close { remaining, errors })?;
            return Ok(ProviderCommand::Close { token, block });
        }
        self.closing = false;
        self.closed = true;
        if !errors.is_empty() {
            return Err(error("cleanup_failed", errors.join("; ")));
        }
        Ok(ProviderCommand::Done { value: Value::Null })
    }
}
