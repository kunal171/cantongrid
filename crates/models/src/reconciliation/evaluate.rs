//! Compare checked integer coefficients directly, without rounding or subtracting to discover order.
use super::{CheckFinding, FixtureResult, Policy, Report, definition::Operator, report};
use crate::ModelError;
use cantongrid_core::value::Value;
use std::cmp::Ordering;

impl Policy {
    /// Evaluate only explicit supplied data. A bad required field makes every check unavailable.
    pub fn evaluate_fixture(&self, payload: &str) -> Report {
        let fields = crate::evaluate::project(&self.definition.fields, payload);
        let checks = self
            .definition
            .checks
            .iter()
            .map(|check| {
                let comparison = (|| {
                    let fields = fields.as_ref().map_err(|e| *e)?;
                    let left = coefficient(fields.get(&check.left).ok_or(ModelError::Definition)?)?;
                    let right =
                        coefficient(fields.get(&check.right).ok_or(ModelError::Definition)?)?;
                    // Comparing -10^38+1 with +10^38-1 must not overflow an intermediate subtraction.
                    Ok::<_, ModelError>(check.operator.accepts(left.cmp(&right)))
                })();
                let (fixture_comparison, error_code) = match comparison {
                    Ok(true) => (FixtureResult::Satisfied, None),
                    Ok(false) => (FixtureResult::Violated, None),
                    Err(error) => (FixtureResult::Unavailable, Some(report::code(error))),
                };
                CheckFinding {
                    name: check.name.clone(),
                    fixture_comparison,
                    error_code,
                }
            })
            .collect();
        Report::fixture(self, checks)
    }
}
/// Scale equality was checked at compilation; the shared evaluator validates all scalar encodings.
fn coefficient(value: &Value) -> Result<i128, ModelError> {
    match value {
        Value::Int64(text) => text
            .parse::<i64>()
            .map(i128::from)
            .map_err(|_| ModelError::Type),
        Value::Numeric(value) => value
            .decimal
            .replace('.', "")
            .parse()
            .map_err(|_| ModelError::Type),
        _ => Err(ModelError::Type),
    }
}
impl Operator {
    /// Closed predicates preserve ordinary exact total ordering; no epsilon or optional/null fallback.
    fn accepts(self, order: Ordering) -> bool {
        match self {
            Self::Eq => order == Ordering::Equal,
            Self::Ne => order != Ordering::Equal,
            Self::Lt => order == Ordering::Less,
            Self::Lte => order != Ordering::Greater,
            Self::Gt => order == Ordering::Greater,
            Self::Gte => order != Ordering::Less,
        }
    }
}
