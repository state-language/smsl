mod operation;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::protocol::{self, HttpRequest, Options, PROTOCOL};
use crate::{
    Diagnostic,
    runtime::system::{Invocation, error},
};

struct Lease {
    id: String,
    epoch: String,
    deadline: u64,
    lifetime: u64,
    ready: bool,
}

struct Operation {
    call: Option<Invocation>,
    fingerprint: [u8; 32],
    session: Option<String>,
    started: u64,
    terminal: Option<String>,
    result: Option<Value>,
    result_fingerprint: Option<[u8; 32]>,
}

fn fingerprint(value: &impl serde::Serialize) -> [u8; 32] {
    Sha256::digest(serde_json::to_vec(value).expect("portable protocol value")).into()
}

pub struct Client {
    options: Options,
    lease: Option<Lease>,
    binding: Option<(String, String)>,
    operations: BTreeMap<String, Operation>,
    active: BTreeSet<String>,
    now: u64,
    status: &'static str,
    request_id: Option<String>,
    latency: Option<u64>,
    last_error: Option<String>,
}

impl Client {
    pub fn new(options: Value) -> Result<Self, Diagnostic> {
        let options: Options = serde_json::from_value(options)
            .map_err(|cause| error("invalid_configuration", cause.to_string()))?;
        if options.contract.is_empty() || options.profile.is_empty() {
            return Err(error(
                "invalid_configuration",
                "Contract and payload profile are required",
            ));
        }
        crate::encoding::validate_value(&json!(options.config))?;
        Ok(Self {
            options,
            lease: None,
            binding: None,
            operations: BTreeMap::new(),
            active: BTreeSet::new(),
            now: 0,
            status: "idle",
            request_id: None,
            latency: None,
            last_error: None,
        })
    }

    fn open(&self) -> Result<(), Diagnostic> {
        if self.status == "closed" {
            return Err(error("closed_resource", "Remote provider is closed"));
        }
        Ok(())
    }

    fn clock(&mut self, now: u64) -> Result<(), Diagnostic> {
        self.open()?;
        if now < self.now {
            return Err(error(
                "invalid_clock",
                "Remote protocol requires a monotonic millisecond clock",
            ));
        }
        self.now = now;
        Ok(())
    }

    fn expired(&mut self) -> Diagnostic {
        self.status = "expired";
        let message = "Provider session expired; no retained result can be assumed";
        self.last_error = Some(message.into());
        error("session_expired", message)
    }

    fn live(&mut self, now: u64) -> Result<&Lease, Diagnostic> {
        self.clock(now)?;
        let lease = self
            .lease
            .as_ref()
            .ok_or_else(|| error("missing_session", "Remote session is not open"))?;
        if now >= lease.deadline {
            return Err(self.expired());
        }
        Ok(self.lease.as_ref().expect("checked lease"))
    }

    pub fn inspect(&self) -> Value {
        json!({"status":self.status,"sessionId":self.lease.as_ref().map(|lease| &lease.id),"requestId":self.request_id,"latencyMs":self.latency,"error":self.last_error})
    }

    pub fn begin(&mut self, call: Invocation, now: u64) -> Result<Value, Diagnostic> {
        self.clock(now)?;
        if call.contract != self.options.contract {
            return Err(error(
                "contract_mismatch",
                "Invocation contract does not match the remote binding",
            ));
        }
        if call.request_id.is_empty() || call.block.is_empty() || call.provider.is_empty() {
            return Err(error(
                "invalid_invocation",
                "Invocation identities must be nonempty",
            ));
        }
        call.basis.validate()?;
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| binding != &(call.block.clone(), call.provider.clone()))
        {
            return Err(error(
                "provider_isolation",
                "A remote provider instance belongs to one block and provider binding",
            ));
        }
        if self.active.contains(&call.request_id) {
            return Err(error(
                "duplicate_request",
                "This request is already pending in the adapter",
            ));
        }
        if let Some(lease) = &self.lease {
            if now >= lease.deadline {
                if self
                    .operations
                    .get(&call.request_id)
                    .is_some_and(|operation| operation.session.as_ref() == Some(&lease.id))
                    || self
                        .active
                        .iter()
                        .any(|id| self.operations[id].session.as_ref() == Some(&lease.id))
                {
                    return Err(self.expired());
                }
                self.lease = None;
            }
        }
        let call_fingerprint = fingerprint(&call);
        if let Some(previous) = self.operations.get(&call.request_id) {
            if previous.session.is_some()
                && previous.session.as_ref() != self.lease.as_ref().map(|lease| &lease.id)
            {
                return Err(error(
                    "request_expired",
                    "An old invocation cannot be replayed in a new session",
                ));
            }
            if previous.fingerprint != call_fingerprint {
                return Err(error(
                    "request_conflict",
                    "A request identity cannot be reused for different invocation data",
                ));
            }
        }
        let needs_session = self.lease.as_ref().is_none_or(|lease| !lease.ready);
        self.binding = Some((call.block.clone(), call.provider.clone()));
        self.request_id = Some(call.request_id.clone());
        self.active.insert(call.request_id.clone());
        match self.operations.entry(call.request_id.clone()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let operation = entry.get_mut();
                operation.started = now;
                operation.call = Some(call);
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Operation {
                    call: Some(call),
                    fingerprint: call_fingerprint,
                    session: None,
                    started: now,
                    terminal: None,
                    result: None,
                    result_fingerprint: None,
                });
            }
        }
        if needs_session {
            self.status = "connecting";
        }
        self.last_error = None;
        Ok(json!({"needs_session":needs_session}))
    }

    pub fn description(
        &mut self,
        response: Value,
        nonce: &str,
        now: u64,
    ) -> Result<HttpRequest, Diagnostic> {
        self.clock(now)?;
        protocol::envelope(&response)?;
        let epoch = protocol::string(&response, "epoch")?;
        let lifetime = protocol::integer(&response, "session_lifetime_ms")?;
        if !(1..=3_600_000).contains(&lifetime) {
            return Err(error(
                "invalid_response",
                "Session lifetime must be within one hour",
            ));
        }
        let capabilities = response
            .get("capabilities")
            .and_then(Value::as_array)
            .ok_or_else(|| error("invalid_response", "Capabilities must be an array"))?;
        if !capabilities.iter().any(|capability| {
            capability.get("contract") == Some(&json!(self.options.contract))
                && capability.get("profile") == Some(&json!(self.options.profile))
                && capability.get("effect") == Some(&json!("pure"))
                && capability.get("persistence") == Some(&json!("stateless"))
        }) {
            return Err(error(
                "unsupported_profile",
                "Provider does not advertise the requested pure, stateless contract/profile",
            ));
        }
        if nonce.is_empty() {
            return Err(error(
                "invalid_configuration",
                "A fresh session nonce is required",
            ));
        }
        let (block, provider) = self.binding.as_ref().ok_or_else(|| {
            error(
                "unknown_request",
                "Begin an invocation before opening a session",
            )
        })?;
        if self.lease.is_some() {
            return Err(error(
                "session_conflict",
                "A remote session is already opening or open",
            ));
        }
        let id = format!("{epoch}.{nonce}");
        if self
            .operations
            .values()
            .any(|operation| operation.session.as_ref() == Some(&id))
        {
            return Err(error(
                "session_conflict",
                "A new remote session requires a fresh identity",
            ));
        }
        let request = HttpRequest::new(
            "PUT",
            format!("/sessions/{}", protocol::segment(&id)),
            Some(
                json!({"protocol":PROTOCOL,"contract":self.options.contract,"profile":self.options.profile,"block":block,"provider":provider,"config":self.options.config}),
            ),
        )?;
        let deadline = now
            .checked_add(lifetime)
            .ok_or_else(|| error("invalid_clock", "Session deadline exceeds the clock range"))?;
        self.lease = Some(Lease {
            id,
            epoch: epoch.into(),
            deadline,
            lifetime,
            ready: false,
        });
        Ok(request)
    }

    pub fn opened(&mut self, response: Value, now: u64) -> Result<(), Diagnostic> {
        protocol::envelope(&response)?;
        let lease = self.live(now)?;
        if response.get("session_id") != Some(&json!(lease.id))
            || response.get("epoch") != Some(&json!(lease.epoch))
            || protocol::integer(&response, "expires_in_ms")? > lease.lifetime
        {
            return Err(error(
                "invalid_response",
                "Session response does not match its creation request",
            ));
        }
        let remaining = protocol::integer(&response, "expires_in_ms")?;
        if remaining == 0 {
            return Err(self.expired());
        }
        let lease = self.lease.as_mut().expect("live lease");
        lease.deadline = lease.deadline.min(now.saturating_add(remaining));
        lease.ready = true;
        self.status = "ready";
        self.last_error = None;
        Ok(())
    }

    pub fn close(&mut self, now: u64) -> Result<Option<HttpRequest>, Diagnostic> {
        if self.status == "closed" {
            return Ok(None);
        }
        self.clock(now)?;
        if !self.active.is_empty() {
            return Err(error(
                "pending_request",
                "Complete or acknowledge cancellation before closing",
            ));
        }
        if let Some(lease) = &self.lease {
            if lease.ready && now < lease.deadline {
                return HttpRequest::new(
                    "DELETE",
                    format!("/sessions/{}", protocol::segment(&lease.id)),
                    None,
                )
                .map(Some);
            }
        }
        self.status = "closed";
        Ok(None)
    }

    pub fn closed(&mut self, response: Option<Value>) -> Result<(), Diagnostic> {
        if !self.active.is_empty() {
            return Err(error(
                "pending_request",
                "Complete or acknowledge cancellation before closing",
            ));
        }
        if let Some(response) = response {
            protocol::envelope(&response)?;
            if response.get("session_id")
                != self.lease.as_ref().map(|lease| json!(lease.id)).as_ref()
                || response.get("status") != Some(&json!("closed"))
            {
                return Err(error("invalid_response", "Invalid close acknowledgment"));
            }
        } else if self
            .lease
            .as_ref()
            .is_some_and(|lease| lease.ready && self.now < lease.deadline)
        {
            return Err(error(
                "invalid_response",
                "Closing a live session requires acknowledgment",
            ));
        }
        self.status = "closed";
        Ok(())
    }

    pub fn close_failed(&mut self, now: u64, code: &str, message: &str) -> Result<(), Diagnostic> {
        self.clock(now)?;
        if !self.active.is_empty() {
            return Err(error(
                "pending_request",
                "Complete or acknowledge cancellation before closing",
            ));
        }
        if !["session_expired", "session_lost", "session_closed"].contains(&code) {
            return Err(error(code, message));
        }
        self.status = "closed";
        Ok(())
    }
}
