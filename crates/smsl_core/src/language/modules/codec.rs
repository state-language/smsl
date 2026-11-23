use crate::{Diagnostic, encoding};

use super::{Module, SourceFormat};

pub fn decode(source: &str, format: SourceFormat) -> Result<Module, Diagnostic> {
    let module = if let Some(format) = format.encoding() {
        let value = encoding::decode(source, format)?;
        serde_json::from_value(value).map_err(|error| {
            Diagnostic::new("syntax", format!("Invalid SMSL module: {error}"), None)
        })?
    } else {
        crate::language::syntax::parser::parse_module(source)?
    };
    validate(&module)?;
    Ok(module)
}

pub fn encode(module: &Module, format: SourceFormat) -> Result<String, Diagnostic> {
    validate(module)?;
    if let Some(format) = format.encoding() {
        encoding::encode(
            &serde_json::to_value(module).expect("module values"),
            format,
        )
    } else {
        let source = crate::language::syntax::serialize::to_smsl_module(module);
        crate::source::lexer::validate_source(&source)?;
        Ok(source)
    }
}

fn validate(module: &Module) -> Result<(), Diagnostic> {
    use crate::language::model::{Datum, Expr};
    super::domains::validate(module)?;
    let mut declarations = module.document();
    declarations.branches.extend(
        module
            .templates
            .iter()
            .map(|template| template.branch.clone()),
    );
    crate::language::limits::check_document(&declarations)?;
    for template in &module.templates {
        super::templates::validate(template)?;
    }
    let reject = || {
        Diagnostic::new(
            "invalid_literal",
            "Definition modules cannot contain event-reference literals",
            None,
        )
    };
    for instance in &module.instances {
        if instance
            .arguments
            .values()
            .any(|value| matches!(value, Datum::Event(_) | Datum::Unknown(_)))
        {
            return Err(Diagnostic::new(
                "invalid_literal",
                "Instance arguments must be known scalar literals",
                None,
            ));
        }
    }
    for branch in &declarations.branches {
        if branch
            .data
            .iter()
            .any(|field| matches!(field.initial, Datum::Event(_)))
            || branch
                .facts
                .iter()
                .any(|fact| matches!(fact.initial, Some(Datum::Event(_))))
        {
            return Err(reject());
        }
        crate::language::definition::expressions::visit(&mut branch.clone(), &mut |expression| {
            if matches!(
                expression,
                Expr::Literal {
                    value: Datum::Event(_)
                }
            ) {
                Err(reject())
            } else {
                Ok(())
            }
        })?;
    }
    Ok(())
}
