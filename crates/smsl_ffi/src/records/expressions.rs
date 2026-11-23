use super::{SmslExprRecord, value};
use crate::boundary::*;
use smsl_core::{Diagnostic, model::Expr};

unsafe fn expr(
    record: *const SmslExprRecord,
    depth: usize,
    nodes: &mut usize,
) -> Result<Expr, Diagnostic> {
    if depth > smsl_core::limits::MAX_EXPRESSION_DEPTH
        || *nodes == smsl_core::limits::MAX_EXPRESSION_NODES
    {
        return Err(Diagnostic::new(
            "resource_limit",
            "Expression exceeds 64 levels or 4096 nodes",
            None,
        ));
    }
    *nodes += 1;
    let r = unsafe { get(record)? };
    unsafe {
        if r.kind == 0 {
            return Ok(Expr::Literal {
                value: value(r.literal)?,
            });
        }
        if r.kind == 1 {
            return Ok(Expr::Read {
                source: text(r.source)?.into(),
                field: text(r.field)?.into(),
            });
        }
        let left = Box::new(expr(r.left, depth + 1, nodes)?);
        if r.kind == 2 {
            return Ok(Expr::Not { expr: left });
        }
        if r.kind == 11 {
            return Ok(Expr::IsUnknown { expr: left });
        }
        let right = Box::new(expr(r.right, depth + 1, nodes)?);
        Ok(match r.kind {
            3 => Expr::And { left, right },
            4 => Expr::Or { left, right },
            5 => Expr::Eq { left, right },
            6 => Expr::Ne { left, right },
            7 => Expr::Lt { left, right },
            8 => Expr::Le { left, right },
            9 => Expr::Gt { left, right },
            10 => Expr::Ge { left, right },
            12 => Expr::Add { left, right },
            13 => Expr::Sub { left, right },
            14 => Expr::Mul { left, right },
            15 => Expr::Div { left, right },
            16 => Expr::Rem { left, right },

            _ => return Err(invalid("Unknown expression kind")),
        })
    }
}
pub(super) unsafe fn expression(record: *const SmslExprRecord) -> Result<Expr, Diagnostic> {
    unsafe { expr(record, 1, &mut 0) }
}

pub(super) unsafe fn optional(record: *const SmslExprRecord) -> Result<Option<Expr>, Diagnostic> {
    if record.is_null() {
        Ok(None)
    } else {
        Ok(Some(unsafe { expression(record)? }))
    }
}
