use serde_json::{Value, json};

use super::{Client, Diagnostic, HttpRequest, PROTOCOL, error, fingerprint, protocol};

impl Client {
    fn operation_path(&mut self, request_id: &str, now: u64) -> Result<String, Diagnostic> {
        let lease = self.live(now)?;
        if !lease.ready {
            return Err(error(
                "missing_session",
                "Remote session creation is not acknowledged",
            ));
        }
        let session_id = lease.id.clone();
        let operation = self
            .operations
            .get(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        if operation
            .session
            .as_ref()
            .is_some_and(|session| session != &session_id)
        {
            return Err(error(
                "request_expired",
                "An old invocation cannot be replayed in a new session",
            ));
        }
        Ok(format!(
            "/sessions/{}/operations/{}",
            protocol::segment(&session_id),
            protocol::segment(request_id)
        ))
    }

    pub fn submit(&mut self, request_id: &str, now: u64) -> Result<HttpRequest, Diagnostic> {
        let path = self.operation_path(request_id, now)?;
        if !self.active.contains(request_id) {
            return Err(error("unknown_request", request_id));
        }
        let operation = &self.operations[request_id];
        let mut invocation = json!(operation.call.as_ref().expect("active invocation"));
        invocation
            .as_object_mut()
            .expect("invocation")
            .remove("config");
        let request = HttpRequest::new(
            "PUT",
            path,
            Some(json!({"protocol":PROTOCOL,"invocation":invocation})),
        )?;
        self.operations
            .get_mut(request_id)
            .expect("operation")
            .session = Some(self.lease.as_ref().expect("lease").id.clone());
        self.status = "pending";
        Ok(request)
    }

    pub fn poll(&mut self, request_id: &str, now: u64) -> Result<HttpRequest, Diagnostic> {
        let path = self.operation_path(request_id, now)?;
        if !self.active.contains(request_id) {
            return Err(error("unknown_request", request_id));
        }
        HttpRequest::new("GET", path, None)
    }

    fn response_identity(
        &mut self,
        request_id: &str,
        response: &Value,
        now: u64,
    ) -> Result<(), Diagnostic> {
        if !self.active.contains(request_id) {
            return Err(error("unknown_request", request_id));
        }
        self.operation_path(request_id, now)?;
        protocol::envelope(response)?;
        if response.get("session_id") != Some(&json!(self.lease.as_ref().expect("lease").id))
            || response.get("request_id") != Some(&json!(request_id))
        {
            return Err(error(
                "invalid_response",
                "Operation identity does not match the invocation",
            ));
        }
        Ok(())
    }

    fn terminal(&mut self, request_id: &str, status: &str) -> Result<(), Diagnostic> {
        if status == "pending" {
            return Ok(());
        }
        let operation = self.operations.get_mut(request_id).expect("operation");
        if operation
            .terminal
            .as_ref()
            .is_some_and(|previous| previous != status)
        {
            return Err(error(
                "invalid_response",
                "Provider changed a terminal operation status",
            ));
        }
        operation.terminal = Some(status.into());
        Ok(())
    }

    fn result(&mut self, request_id: &str, result: Value) -> Result<Value, Diagnostic> {
        let operation = self.operations.get_mut(request_id).expect("operation");
        let result_fingerprint = fingerprint(&result);
        if operation
            .result_fingerprint
            .is_some_and(|previous| previous != result_fingerprint)
        {
            return Err(error(
                "invalid_response",
                "Provider changed a terminal operation result",
            ));
        }
        operation.result = Some(result.clone());
        operation.result_fingerprint = Some(result_fingerprint);
        Ok(result)
    }

    pub fn operation(
        &mut self,
        request_id: &str,
        response: Value,
        now: u64,
    ) -> Result<Value, Diagnostic> {
        self.response_identity(request_id, &response, now)?;
        let status = protocol::string(&response, "status")?;
        let value = match status {
            "pending" => match self.operations[request_id].result.clone() {
                Some(result) if result["status"] == "failed" => {
                    return Err(protocol::remote_error(&result)?);
                }
                Some(result) => return Ok(result),
                None if self.operations[request_id].terminal.as_deref() == Some("cancelled") => {
                    return Ok(json!({"status":"cancelled"}));
                }
                None => json!({"status":"pending"}),
            },
            "completed" => {
                json!({"status":"completed","outputs":protocol::outputs(&response, &self.operations[request_id].call.as_ref().expect("active invocation").basis)?})
            }
            "cancelled" => json!({"status":"cancelled"}),
            "failed" => {
                let cause = protocol::remote_error(&response)?;
                self.terminal(request_id, status)?;
                self.result(
                    request_id,
                    json!({"status":"failed","error":{"code":cause.code,"message":cause.message}}),
                )?;
                self.status = "failed";
                return Err(cause);
            }
            _ => return Err(error("invalid_response", "Unknown operation status")),
        };
        self.terminal(request_id, status)?;
        if status != "pending" {
            self.result(request_id, value.clone())?;
        }
        self.status = match status {
            "completed" => "completed",
            "cancelled" => "cancelled",
            _ => "pending",
        };
        self.last_error = None;
        Ok(value)
    }

    pub fn cancel(
        &mut self,
        request_id: &str,
        now: u64,
    ) -> Result<Option<HttpRequest>, Diagnostic> {
        self.clock(now)?;
        if !self.active.contains(request_id) || self.operations[request_id].terminal.is_some() {
            return Ok(None);
        }
        let path = self.operation_path(request_id, now)?;
        HttpRequest::new(
            "POST",
            format!("{path}/cancel"),
            Some(json!({"protocol":PROTOCOL})),
        )
        .map(Some)
    }

    pub fn cancelled(
        &mut self,
        request_id: &str,
        response: Value,
        now: u64,
    ) -> Result<bool, Diagnostic> {
        self.response_identity(request_id, &response, now)?;
        let status = protocol::string(&response, "status")?;
        if !["pending", "completed", "failed", "cancelled"].contains(&status) {
            return Err(error("invalid_response", "Unknown cancellation status"));
        }
        if status != "pending" {
            self.terminal(request_id, status)?;
        }
        if status == "cancelled" {
            self.status = "cancelled";
        }
        Ok(status == "cancelled")
    }

    pub fn retry(&mut self, now: u64, message: &str) -> Result<(), Diagnostic> {
        self.live(now)?;
        self.status = "reconnecting";
        self.last_error = Some(message.into());
        Ok(())
    }

    pub fn cancel_retry(
        &mut self,
        request_id: &str,
        now: u64,
        code: &str,
        message: &str,
    ) -> Result<bool, Diagnostic> {
        self.live(now)?;
        if code != "unknown_operation" {
            return Err(error(code, message));
        }
        Ok(self.active.contains(request_id) && self.operations[request_id].terminal.is_none())
    }

    pub fn finish(
        &mut self,
        request_id: &str,
        now: u64,
        failure: Option<(String, String)>,
    ) -> Result<(), Diagnostic> {
        self.clock(now)?;
        let operation = self
            .operations
            .get(request_id)
            .ok_or_else(|| error("unknown_request", request_id))?;
        if failure.is_none() && operation.terminal.is_none() {
            return Err(error(
                "pending_request",
                "A pending operation cannot finish without a terminal result or error",
            ));
        }
        if !self.active.remove(request_id) {
            return Err(error("unknown_request", request_id));
        }
        self.latency = Some(now - operation.started);
        let operation = self
            .operations
            .get_mut(request_id)
            .expect("active invocation");
        operation.call = None;
        operation.result = None;
        if let Some((code, message)) = failure {
            self.last_error = Some(message);
            if ["session_expired", "session_lost"].contains(&code.as_str()) {
                self.status = "expired";
                if let Some(lease) = &mut self.lease {
                    lease.deadline = 0;
                }
            } else if !["expired", "cancelled"].contains(&self.status) {
                self.status = "failed";
            }
        }
        if self.active.is_empty() && self.lease.as_ref().is_some_and(|lease| !lease.ready) {
            self.lease = None;
        }
        Ok(())
    }
}
