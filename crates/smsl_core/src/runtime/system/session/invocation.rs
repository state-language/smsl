use serde_json::json;

use super::{PendingInvocation, Session};
use crate::{
    Diagnostic,
    architecture::Effect,
    runtime::system::{Invocation, Records, error},
};

impl Session {
    pub fn check_current_basis(
        &self,
        request_id: &str,
        current_basis: &super::Basis,
    ) -> Result<(), Diagnostic> {
        current_basis.validate()?;
        let call = self
            .pending(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        if &call.basis != current_basis {
            return Err(error(
                "stale_basis",
                "Provider result has a stale model or knowledge basis",
            ));
        }
        Ok(())
    }

    pub fn complete_checked(
        &mut self,
        request_id: &str,
        outputs: Records,
        current_basis: &super::Basis,
    ) -> Result<Records, Diagnostic> {
        self.check_current_basis(request_id, current_basis)?;
        self.complete(request_id, outputs)
    }

    pub fn fail_if_pending(&mut self, request_id: &str, message: &str) -> bool {
        if !self.pending.contains_key(request_id) {
            return false;
        }
        self.fail(request_id, message).expect("pending invocation");
        true
    }

    pub fn cancel_checked(
        &mut self,
        request_id: &str,
        acknowledged: bool,
    ) -> Result<(), Diagnostic> {
        if !self.pending.contains_key(request_id) {
            return Err(error("unknown_request", request_id));
        }
        if !acknowledged {
            return Err(error(
                "cancellation_unacknowledged",
                "Provider did not acknowledge cancellation",
            ));
        }
        self.cancel(request_id)
    }

    pub fn assert_idle(&self) -> Result<(), Diagnostic> {
        if !self.pending.is_empty() {
            return Err(error(
                "pending_request",
                "Complete or cancel pending requests before closing",
            ));
        }
        Ok(())
    }

    pub fn prepare(
        &mut self,
        block: &str,
        request_id: &str,
        inputs: Records,
        allow_effects: bool,
    ) -> Result<Invocation, Diagnostic> {
        if request_id.trim().is_empty() || self.requests.contains(request_id) {
            return Err(error(
                "duplicate_request",
                "Request identity must be nonempty and unique within the session",
            ));
        }
        if self
            .pending
            .values()
            .any(|pending| pending.call.block == block)
        {
            return Err(error(
                "pending_invocation",
                "A block already has an outstanding invocation",
            ));
        }
        let definition = self
            .definition
            .block(block)
            .ok_or_else(|| error("unknown_block", block))?;
        let provider = definition
            .provider
            .clone()
            .ok_or_else(|| error("unbound_block", block))?;
        let contract = &self.contracts[&definition.contract];
        if contract.effect != Effect::Pure && !allow_effects {
            return Err(error(
                "effect_not_authorized",
                "Evolution and execution calls require explicit allow_effects",
            ));
        }
        for (name, record) in &inputs {
            let port = contract
                .inputs
                .get(name)
                .ok_or_else(|| error("unknown_input", name))?;
            record.validate(port, &self.basis)?;
            self.sequence(&format!("{block}.{name}"), record)?;
        }
        let mut resolved = Records::new();
        for (name, port) in &contract.inputs {
            let target = format!("{block}.{name}");
            match inputs.get(name).or_else(|| self.inputs.get(&target)) {
                Some(record) => {
                    record.validate(port, &self.basis)?;
                    resolved.insert(name.clone(), record.clone());
                }
                None if port.required => return Err(error("missing_input", target)),
                None => {}
            }
        }
        let invocation = Invocation {
            request_id: request_id.into(),
            block: block.into(),
            provider,
            config: definition.config.clone(),
            contract: definition.contract.clone(),
            inputs: resolved,
            basis: self.basis.clone(),
            allow_effects,
        };
        for (name, record) in inputs {
            let target = format!("{block}.{name}");
            self.sequences.insert(target.clone(), record.sequence);
            self.delayed.remove(&target);
            self.inputs.insert(target, record);
        }
        self.requests.insert(request_id.into());
        self.pending.insert(
            request_id.into(),
            PendingInvocation {
                call: invocation.clone(),
                cancelling: false,
            },
        );
        self.record_invocation("call", &invocation, Some(json!(invocation)));
        Ok(invocation)
    }

    pub fn complete(&mut self, request_id: &str, outputs: Records) -> Result<Records, Diagnostic> {
        let call = self
            .pending(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        if call.basis != self.basis {
            return Err(error(
                "stale_result",
                "Invocation basis changed before completion",
            ));
        }
        let contract = &self.contracts[&call.contract];
        for (name, port) in &contract.outputs {
            if port.required && !outputs.contains_key(name) {
                return Err(error("missing_output", name));
            }
        }
        let mut immediate = Vec::new();
        let mut delayed = Vec::new();
        for (name, record) in &outputs {
            let port = contract
                .outputs
                .get(name)
                .ok_or_else(|| error("unknown_output", name))?;
            record.validate(port, &call.basis)?;
            if record.basis != call.basis {
                return Err(error(
                    "stale_result",
                    "Output must identify its invocation basis",
                ));
            }
            let source = format!("{}.{name}", call.block);
            self.sequence(&format!("output:{source}"), record)?;
            for connection in self
                .definition
                .connections
                .iter()
                .filter(|edge| edge.from == source)
            {
                record.validate(self.input_port(&connection.to)?, &self.basis)?;
                self.sequence(&connection.to, record)?;
                if connection.delay == 0 {
                    immediate.push((connection.to.clone(), record.clone()));
                } else {
                    delayed.push((connection.to.clone(), record.clone()));
                }
            }
        }
        let block = call.block.clone();
        for (name, record) in &outputs {
            self.sequences
                .insert(format!("output:{block}.{name}"), record.sequence);
        }
        for (target, record) in immediate {
            self.sequences.insert(target.clone(), record.sequence);
            self.delayed.remove(&target);
            self.inputs.insert(target, record);
        }
        for (target, record) in delayed {
            self.sequences.insert(target.clone(), record.sequence);
            self.delayed.insert(target, record);
        }
        let pending = self.pending.remove(request_id).expect("pending invocation");
        self.record_invocation("output", &pending.call, Some(json!(outputs)));
        Ok(outputs)
    }

    pub fn fail(&mut self, request_id: &str, message: &str) -> Result<(), Diagnostic> {
        let pending = self
            .pending
            .remove(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        self.record_invocation("failure", &pending.call, Some(json!({"message": message})));
        Ok(())
    }

    pub fn cancel(&mut self, request_id: &str) -> Result<(), Diagnostic> {
        let pending = self
            .pending
            .remove(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        self.record_invocation("cancelled", &pending.call, None);
        Ok(())
    }
}
