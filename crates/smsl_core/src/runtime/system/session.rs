mod invocation;
mod lifecycle;
mod protocol;
mod trace;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{
    Basis, Contract, Freshness, Invocation, Port, Record, SystemDefinition, TraceEntry, error,
    validate,
};
use crate::Diagnostic;
use crate::architecture::validation::{contracts, endpoint};

pub struct Session {
    definition: SystemDefinition,
    contracts: BTreeMap<String, Contract>,
    basis: Basis,
    inputs: BTreeMap<String, Record>,
    delayed: BTreeMap<String, Record>,
    sequences: BTreeMap<String, u64>,
    pending: BTreeMap<String, PendingInvocation>,
    requests: BTreeSet<String>,
    trace: trace::TraceLog,
}

struct PendingInvocation {
    call: Invocation,
    cancelling: bool,
}

pub use lifecycle::{
    InvocationPhase, InvocationStatus, LifecycleEntry, LifecycleKind, LifecyclePage,
    LifecycleStatus,
};

impl Session {
    pub fn new(definition: SystemDefinition, basis: Basis) -> Result<Self, Diagnostic> {
        validate(&definition)?;
        basis.validate()?;
        Ok(Self {
            contracts: contracts(&definition)?,
            definition,
            basis,
            inputs: BTreeMap::new(),
            delayed: BTreeMap::new(),
            sequences: BTreeMap::new(),
            pending: BTreeMap::new(),
            requests: BTreeSet::new(),
            trace: trace::TraceLog::default(),
        })
    }

    pub fn definition(&self) -> &SystemDefinition {
        &self.definition
    }
    pub fn basis(&self) -> &Basis {
        &self.basis
    }
    pub fn pending(&self, request_id: &str) -> Option<&Invocation> {
        self.pending.get(request_id).map(|pending| &pending.call)
    }
    pub fn trace(&self) -> &[TraceEntry] {
        self.trace.entries()
    }

    pub fn set_input(&mut self, target: &str, record: Record) -> Result<(), Diagnostic> {
        let port = self.input_port(target)?;
        record.validate(port, &self.basis)?;
        self.sequence(target, &record)?;
        self.sequences.insert(target.into(), record.sequence);
        self.delayed.remove(target);
        self.inputs.insert(target.into(), record.clone());
        self.record("input", None, None, Some(target), Some(json!(record)));
        Ok(())
    }

    pub fn advance(&mut self, basis: Basis) -> Result<(), Diagnostic> {
        basis.validate()?;
        let rebase = basis.revision != self.basis.revision;
        if rebase && !self.pending.is_empty() {
            return Err(error(
                "pending_invocation",
                "Resolve outstanding calls before changing revision",
            ));
        }
        if !rebase && self.basis.tick.checked_add(1) != Some(basis.tick) {
            return Err(error(
                "invalid_tick",
                "Advance exactly one tick, or rebase to a new revision",
            ));
        }
        if rebase {
            self.inputs.clear();
            self.delayed.clear();
            self.sequences.clear();
        } else {
            let retained: BTreeMap<_, _> = self
                .inputs
                .iter()
                .filter(|(target, _)| {
                    self.input_port(target).expect("validated input").freshness == Freshness::Any
                })
                .map(|(target, value)| (target.clone(), value.clone()))
                .collect();
            self.inputs = retained;
            self.inputs.append(&mut self.delayed);
        }
        self.basis = basis;
        self.record(
            if rebase { "rebase" } else { "advance" },
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    pub fn inspect(&self) -> Value {
        json!({"basis":self.basis, "pending":self.pending.values().map(|pending| &pending.call).collect::<Vec<_>>(), "trace":self.trace.entries()})
    }

    pub fn status(&self) -> Value {
        json!({"basis":self.basis, "pending":self.pending.values().map(|pending| &pending.call).collect::<Vec<_>>()})
    }

    pub fn read_trace(&self, cursor: u64, limit: u64) -> Result<super::TracePage, Diagnostic> {
        self.trace.read(cursor, limit)
    }

    pub fn set_trace_retention(&mut self, limit: Option<u64>) -> Result<(), Diagnostic> {
        self.trace.retain(limit)
    }

    fn input_port(&self, target: &str) -> Result<&Port, Diagnostic> {
        let (block, name) = endpoint(target)?;
        let block = self
            .definition
            .block(block)
            .ok_or_else(|| error("unknown_block", block))?;
        self.contracts[&block.contract]
            .inputs
            .get(name)
            .ok_or_else(|| error("unknown_input", target))
    }

    fn sequence(&self, endpoint: &str, record: &Record) -> Result<(), Diagnostic> {
        if self
            .sequences
            .get(endpoint)
            .is_some_and(|previous| *previous >= record.sequence)
        {
            return Err(error(
                "out_of_order",
                format!("{endpoint} requires a strictly increasing sequence"),
            ));
        }
        Ok(())
    }

    fn record(
        &mut self,
        kind: &str,
        block: Option<&str>,
        request: Option<&str>,
        endpoint: Option<&str>,
        detail: Option<Value>,
    ) {
        self.trace.push(TraceEntry {
            index: self.trace.next(),
            kind: kind.into(),
            basis: self.basis.clone(),
            invocation_basis: None,
            block: block.map(str::to_owned),
            request_id: request.map(str::to_owned),
            endpoint: endpoint.map(str::to_owned),
            detail,
        });
    }

    fn record_invocation(&mut self, kind: &str, invocation: &Invocation, detail: Option<Value>) {
        self.trace.push(TraceEntry {
            index: self.trace.next(),
            kind: kind.into(),
            basis: self.basis.clone(),
            invocation_basis: Some(invocation.basis.clone()),
            block: Some(invocation.block.clone()),
            request_id: Some(invocation.request_id.clone()),
            endpoint: None,
            detail,
        });
    }
}
