use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic,
    definition::expressions,
    model::{Branch, Expr},
};

pub(super) fn rewrite(
    branch: &mut Branch,
    names: &BTreeMap<String, String>,
) -> Result<BTreeSet<String>, Diagnostic> {
    rewrite_inner(branch, names, false)
}

pub(super) fn rewrite_template(
    branch: &mut Branch,
    names: &BTreeMap<String, String>,
) -> Result<BTreeSet<String>, Diagnostic> {
    rewrite_inner(branch, names, true)
}

fn rewrite_inner(
    branch: &mut Branch,
    names: &BTreeMap<String, String>,
    parameters: bool,
) -> Result<BTreeSet<String>, Diagnostic> {
    let mut dependencies = BTreeSet::new();
    let resolve = |name: &str| {
        names.get(name).cloned().ok_or_else(|| {
            super::linker::error(format!(
                "Unknown branch reference {name:?} in {:?}",
                branch.name
            ))
        })
    };
    for binding in &mut branch.sub_branches {
        binding.branch = resolve(&binding.branch)?;
        dependencies.insert(binding.branch.clone());
    }
    for read in &mut branch.reads {
        read.branch = resolve(&read.branch)?;
        dependencies.insert(read.branch.clone());
    }
    let aliases: BTreeSet<_> = branch.reads.iter().map(|read| read.alias.clone()).collect();
    let branch_name = branch.name.clone();
    expressions::visit(branch, &mut |expression| {
        if let Expr::Read { source, .. } = expression
            && !matches!(source.as_str(), "self" | "event" | "payload" | "args")
            && !aliases.contains(source)
            && !(parameters && source == "params")
        {
            *source = names.get(source).cloned().ok_or_else(|| {
                super::linker::error(format!(
                    "Unknown branch reference {source:?} in {branch_name:?}"
                ))
            })?;
            dependencies.insert(source.clone());
        }
        Ok(())
    })?;
    Ok(dependencies)
}
