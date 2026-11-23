use std::collections::{BTreeMap, BTreeSet};

use super::{Domain, Module};
use crate::{Diagnostic, model::Branch};

pub(super) fn validate(module: &Module) -> Result<(), Diagnostic> {
    let mut names = BTreeSet::new();
    let count: usize = module
        .domains
        .iter()
        .map(|domain| 1 + domain.values.len())
        .sum();
    if count > crate::limits::MAX_DECLARATIONS {
        return Err(Diagnostic::new(
            "resource_limit",
            "Domains exceed 100000 declarations",
            None,
        ));
    }
    for (index, domain) in module.domains.iter().enumerate() {
        if domain.name.is_empty() || !names.insert(&domain.name) {
            return Err(
                super::linker::error("Domain names must be nonempty and unique")
                    .at_path(format!("/domains/{index}/name")),
            );
        }
        crate::language::validation::domain(&domain.values, domain.range, None)
            .map_err(|error| error.prepend_path(&format!("/domains/{index}")))?;
    }
    for branch in module
        .branches
        .iter()
        .chain(module.templates.iter().map(|template| &template.branch))
    {
        for fact in &branch.facts {
            if let Some(name) = &fact.domain
                && (name.is_empty() || !fact.values.is_empty() || fact.range.is_some())
            {
                return Err(Diagnostic::new(
                    "invalid_domain",
                    "A named domain requires a nonempty name, empty values and no range",
                    fact.location,
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn resolve(
    branch: &mut Branch,
    symbols: &BTreeMap<String, String>,
    domains: &BTreeMap<String, Domain>,
    expanded_bytes: &mut usize,
) -> Result<(), Diagnostic> {
    for fact in &mut branch.facts {
        let Some(name) = fact.domain.take() else {
            continue;
        };
        let domain = symbols
            .get(&name)
            .and_then(|target| domains.get(target))
            .ok_or_else(|| {
                Diagnostic::new(
                    "unknown_reference",
                    format!("Unknown domain {name:?} for fact {:?}", fact.name),
                    fact.location,
                )
            })?;
        *expanded_bytes += serde_json::to_vec(domain).expect("domain record").len();
        if *expanded_bytes > crate::limits::MAX_INPUT_BYTES {
            return Err(Diagnostic::new(
                "resource_limit",
                "Expanded named domains exceed 8 MiB",
                fact.location,
            ));
        }
        fact.values.clone_from(&domain.values);
        fact.range = domain.range;
    }
    Ok(())
}
