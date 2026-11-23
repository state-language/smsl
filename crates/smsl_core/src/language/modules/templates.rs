use super::{Template, linker::error};
use crate::{
    Diagnostic,
    definition::expressions,
    model::{Branch, DataType, Datum, Expr},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate(template: &Template) -> Result<(), Diagnostic> {
    let mut names = BTreeSet::new();
    for field in &template.parameters {
        if field.name.is_empty() || !names.insert(&field.name) {
            return Err(error(
                "Template parameter names must be nonempty and unique",
            ));
        }
        if field.data_type == DataType::Event {
            return Err(error(
                "Template parameters cannot contain runtime event references",
            ));
        }
    }
    if template
        .branch
        .reads
        .iter()
        .any(|read| read.alias == "params")
    {
        return Err(error(
            "The params source is reserved inside parameterized branches",
        ));
    }
    expressions::visit(&mut template.branch.clone(), &mut |expression| {
        if let Expr::Read { source, field } = expression
            && source == "params"
            && !names.contains(field)
        {
            return Err(error(format!("Unknown template parameter {field:?}")));
        }
        Ok(())
    })
}

pub(super) fn instantiate(
    template: &Template,
    name: &str,
    arguments: &BTreeMap<String, Datum>,
) -> Result<Branch, Diagnostic> {
    validate(template)?;
    if arguments.len() != template.parameters.len() {
        return Err(error(format!(
            "Instance {name:?} requires exactly its declared parameters"
        )));
    }
    for field in &template.parameters {
        let value = arguments
            .get(&field.name)
            .ok_or_else(|| error(format!("Missing parameter {:?}", field.name)))?;
        if !(matches!(value, Datum::Null(())) && field.nullable || field.data_type.accepts(value)) {
            return Err(error(format!(
                "Invalid value for parameter {:?}",
                field.name
            )));
        }
    }
    let mut branch = prototype(template, name)?;
    expressions::visit(&mut branch, &mut |expression| {
        if let Expr::Read { source, field } = expression
            && source == "params"
        {
            *expression = Expr::Literal {
                value: arguments[field].clone(),
            };
        }
        Ok(())
    })?;
    Ok(branch)
}

pub(super) fn prototype(template: &Template, name: &str) -> Result<Branch, Diagnostic> {
    let mut branch = template.branch.clone();
    branch.name = name.into();
    expressions::visit(&mut branch, &mut |expression| {
        if let Expr::Read { source, .. } = expression
            && source != "params"
            && source == &template.branch.name
        {
            *source = name.into();
        }
        Ok(())
    })?;
    Ok(branch)
}
