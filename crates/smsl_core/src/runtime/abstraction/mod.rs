mod condition;
mod types;

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::language::semantics::Evaluation;
use crate::runtime::{RuntimeState, validation::error};
use crate::{
    Diagnostic, Machine,
    model::{Datum, Value},
};

pub use types::{AbstractionValue, ConcreteInput, Concretization, SnapshotBasis};

impl RuntimeState {
    pub fn abstraction(
        &self,
        machine: &Machine,
        branch: &str,
        name: &str,
    ) -> Result<AbstractionValue, Diagnostic> {
        self.check_machine(machine)?;
        let mapping = machine.abstraction(branch, name)?;
        let current = self.branch(branch).expect("compiled branch");
        Ok(AbstractionValue {
            revision: machine.revision().into(),
            basis: self.query_basis()?,
            branch: branch.into(),
            abstraction: name.into(),
            facts: mapping
                .facts
                .iter()
                .map(|name| (name.clone(), current.facts[name].clone()))
                .collect(),
        })
    }

    pub fn concretize(
        &self,
        machine: &Machine,
        branch: &str,
        name: &str,
        requirements: &BTreeMap<String, Value>,
        knowledge: bool,
    ) -> Result<Concretization, Diagnostic> {
        self.check_machine(machine)?;
        let mapping = machine.abstraction(branch, name)?;
        let definition =
            &machine.document().branches[machine.branch_id(branch).expect("compiled branch")];
        let mut condition = condition::Condition::new(machine);
        let mut terms = Vec::new();
        for (field, value) in requirements {
            if !mapping.facts.contains(field) {
                return Err(error(
                    "invalid_concretization",
                    format!("Fact {field:?} is not an output of abstraction {name:?}"),
                ));
            }
            let fact = definition
                .facts
                .iter()
                .find(|fact| fact.name == *field)
                .expect("compiled output");
            if !fact.contains(value) {
                return Err(error(
                    "invalid_concretization",
                    format!("Requirement for {field:?} is outside its domain"),
                ));
            }
            terms.push(condition::equal(
                condition.field(definition, field, 1)?,
                value,
            ));
        }
        if knowledge {
            terms.extend(condition::knowledge(&mut condition, self));
        }
        let query = Concretization {
            version: "1".into(),
            revision: machine.revision().into(),
            branch: branch.into(),
            abstraction: name.into(),
            requirements: requirements.clone(),
            basis: if knowledge {
                Some(self.query_basis()?)
            } else {
                None
            },
            condition: condition::conjunction(terms)?,
            inputs: condition.inputs(),
        };
        let encoded = serde_json::to_string(&query)
            .map_err(|failure| error("invalid_concretization", failure.to_string()))?;
        crate::limits::input(&encoded, false)?;
        Ok(query)
    }

    pub fn validate_concretization(
        &self,
        machine: &Machine,
        query: &Concretization,
    ) -> Result<(), Diagnostic> {
        self.check_machine(machine)?;
        if query.revision != machine.revision() {
            return Err(error(
                "stale_concretization",
                "Concretization does not match the current model or knowledge snapshot",
            ));
        }
        if let Some(basis) = &query.basis {
            if basis != &self.query_basis()? {
                return Err(error(
                    "stale_concretization",
                    "Concretization does not match the current knowledge snapshot",
                ));
            }
        }
        query.validate_record()?;
        let expected = self.concretize(
            machine,
            &query.branch,
            &query.abstraction,
            &query.requirements,
            query.basis.is_some(),
        )?;
        if query != &expected {
            return Err(error(
                "invalid_concretization",
                "Concretization does not match its canonical mapping and assumptions",
            ));
        }
        Ok(())
    }

    pub fn matches_concretization(
        &self,
        machine: &Machine,
        query: &Concretization,
        candidate: Option<&RuntimeState>,
    ) -> Result<Datum, Diagnostic> {
        self.validate_concretization(machine, query)?;
        let candidate = candidate.unwrap_or(self);
        candidate.check_machine(machine)?;
        let branch = &machine.document().branches
            [machine.branch_id(&query.branch).expect("validated query")];
        Evaluation::resolved(&candidate.branches, branch).predicate(&query.condition)
    }

    fn query_basis(&self) -> Result<SnapshotBasis, Diagnostic> {
        let encoded = serde_json::to_vec(&self.snapshot())
            .map_err(|failure| error("invalid_snapshot", failure.to_string()))?;
        Ok(SnapshotBasis {
            context: self.context.clone(),
            generation: self.generation,
            step: self.step,
            time: self.time,
            fingerprint: format!("{:x}", Sha256::digest(encoded)),
        })
    }
}
