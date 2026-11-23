use super::Parser;
use crate::diagnostic::Diagnostic;
use crate::language::model::{Datum, Expr};
use crate::source::lexer::Kind;

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expr, Diagnostic> {
        self.expression_nodes = 0;
        self.expression_at(0)
    }

    fn expression_at(&mut self, minimum: u8) -> Result<Expr, Diagnostic> {
        if self.expression_depth >= 2 * crate::limits::MAX_NESTING {
            return Err(Diagnostic::new(
                "resource_limit",
                "Expression parsing exceeds 192 levels",
                Some(self.current().location),
            ));
        }
        self.expression_depth += 1;
        let result = self.expression_inner(minimum);
        self.expression_depth -= 1;
        result
    }

    fn expression_inner(&mut self, minimum: u8) -> Result<Expr, Diagnostic> {
        self.expression_nodes += 1;
        if self.expression_nodes > crate::limits::MAX_EXPRESSION_NODES {
            return Err(Diagnostic::new(
                "resource_limit",
                "Expression exceeds 4096 nodes",
                Some(self.current().location),
            ));
        }
        let mut left = if self.keyword("is_unknown") {
            self.expect(Kind::OpenMap)?;
            let expr = self.expression_at(0)?;
            self.expect(Kind::CloseMap)?;
            Expr::IsUnknown {
                expr: Box::new(expr),
            }
        } else if self.keyword("not") {
            Expr::Not {
                expr: Box::new(self.expression_at(6)?),
            }
        } else if self.consume(&Kind::OpenMap) {
            let expr = self.expression_at(0)?;
            self.expect(Kind::CloseMap)?;
            expr
        } else if self
            .tokens
            .get(self.index + 1)
            .is_some_and(|token| token.kind == Kind::Dot)
        {
            let source = if self.keyword("event") {
                "event".into()
            } else {
                self.name()?.0
            };
            self.expect(Kind::Dot)?;
            let field = if self.keyword("state") {
                "state".into()
            } else if source == "event" && self.keyword("time") {
                "time".into()
            } else {
                self.name()?.0
            };
            Expr::Read { source, field }
        } else {
            Expr::Literal {
                value: self.datum()?,
            }
        };
        loop {
            let precedence = match &self.current().kind {
                Kind::Name(name) if name == "or" => 1,
                Kind::Name(name) if name == "and" => 2,
                Kind::Eq | Kind::Ne | Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge => 3,
                Kind::Plus | Kind::Minus => 4,
                Kind::Star | Kind::Slash | Kind::Percent => 5,
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.expression_nodes += 1;
            let operator = self.take().kind;
            let right = Box::new(self.expression_at(precedence + 1)?);
            let lhs = Box::new(left);
            left = match operator {
                Kind::Name(name) if name == "or" => Expr::Or { left: lhs, right },
                Kind::Name(_) => Expr::And { left: lhs, right },
                Kind::Eq => Expr::Eq { left: lhs, right },
                Kind::Ne => Expr::Ne { left: lhs, right },
                Kind::Lt => Expr::Lt { left: lhs, right },
                Kind::Le => Expr::Le { left: lhs, right },
                Kind::Gt => Expr::Gt { left: lhs, right },
                Kind::Ge => Expr::Ge { left: lhs, right },
                Kind::Plus => Expr::Add { left: lhs, right },
                Kind::Minus => Expr::Sub { left: lhs, right },
                Kind::Star => Expr::Mul { left: lhs, right },
                Kind::Slash => Expr::Div { left: lhs, right },
                Kind::Percent => Expr::Rem { left: lhs, right },
                _ => unreachable!("operator was recognized"),
            };
            left = crate::language::limits::checked_expression(left)?;
        }
        crate::language::limits::checked_expression(left)
    }

    pub(super) fn datum(&mut self) -> Result<Datum, Diagnostic> {
        match self.current().kind.clone() {
            Kind::Name(name) if name == "unknown" => {
                self.take();
                Ok(Datum::unknown())
            }
            Kind::Null => {
                self.take();
                Ok(Datum::Null(()))
            }
            Kind::Bool(value) => {
                self.take();
                Ok(Datum::Bool(value))
            }
            Kind::String(value) => {
                self.take();
                Ok(Datum::String(value))
            }
            Kind::Number(value) => {
                self.take();
                Ok(Datum::Time(value))
            }
            _ => Err(self.error("expected a literal or field read")),
        }
    }
}
