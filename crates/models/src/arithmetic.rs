//! Checked fixed-scale arithmetic: no floating point, implicit rounding or saturating overflow.
use crate::ModelError;
use cantongrid_core::value::{Decimal, Value};

/// Normalize a source decimal string to declared scale without discarding any digits.
/// For example, "1.2" at scale 3 becomes "1.200", while "1.2345" is rejected.
pub(crate) fn decimal(text: &str, scale: u8) -> Result<Decimal, ModelError> {
    if scale > 37 || text.len() > 80 {
        return Err(ModelError::Type);
    }
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || (whole.len() > 1 && whole.starts_with('0'))
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > scale as usize
        || (unsigned.contains('.') && fraction.is_empty())
        || whole.trim_start_matches('0').len() + scale as usize > 38
    {
        return Err(ModelError::Type);
    }
    let mut digits = format!("{whole}{fraction}");
    digits.extend(std::iter::repeat_n('0', scale as usize - fraction.len()));
    let magnitude = digits.parse::<i128>().map_err(|_| ModelError::Type)?;
    let coefficient = if text.starts_with('-') {
        -magnitude
    } else {
        magnitude
    };
    fixed(coefficient, scale).map_err(|_| ModelError::Type)
}

/// Render the integer coefficient with its fixed decimal point; validate the Daml bound.
fn fixed(coefficient: i128, scale: u8) -> Result<Decimal, ModelError> {
    let mut digits = coefficient.unsigned_abs().to_string();
    let decimal = if scale == 0 {
        digits
    } else {
        if digits.len() <= scale as usize {
            digits = format!(
                "{}{}",
                "0".repeat(scale as usize + 1 - digits.len()),
                digits
            );
        }
        let split = digits.len() - scale as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    let decimal = if coefficient < 0 {
        format!("-{decimal}")
    } else {
        decimal
    };
    let value = Decimal { scale, decimal };
    Value::Numeric(value.clone())
        .validate()
        .map_err(|_| ModelError::Overflow)?;
    Ok(value)
}

fn coefficient(decimal: &Decimal) -> Result<i128, ModelError> {
    decimal
        .decimal
        .replace('.', "")
        .parse()
        .map_err(|_| ModelError::Overflow)
}

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Add,
    Subtract,
    Multiply,
}

/// Type checking precedes evaluation, but this match still denies every unsupported pair.
pub(crate) fn calculate(
    operation: Operation,
    left: Value,
    right: Value,
) -> Result<Value, ModelError> {
    let checked = |a: i128, b: i128| {
        match operation {
            Operation::Add => a.checked_add(b),
            Operation::Subtract => a.checked_sub(b),
            Operation::Multiply => a.checked_mul(b),
        }
        .ok_or(ModelError::Overflow)
    };
    match (left, right) {
        (Value::Int64(a), Value::Int64(b)) => {
            let value = checked(
                a.parse().map_err(|_| ModelError::Type)?,
                b.parse().map_err(|_| ModelError::Type)?,
            )?;
            Ok(Value::Int64(
                i64::try_from(value)
                    .map_err(|_| ModelError::Overflow)?
                    .to_string(),
            ))
        }
        (Value::Numeric(a), Value::Numeric(b))
            if a.scale == b.scale && !matches!(operation, Operation::Multiply) =>
        {
            Ok(Value::Numeric(fixed(
                checked(coefficient(&a)?, coefficient(&b)?)?,
                a.scale,
            )?))
        }
        (Value::Numeric(a), Value::Int64(b)) | (Value::Int64(b), Value::Numeric(a))
            if matches!(operation, Operation::Multiply) =>
        {
            Ok(Value::Numeric(fixed(
                checked(
                    coefficient(&a)?,
                    b.parse::<i64>().map_err(|_| ModelError::Type)? as i128,
                )?,
                a.scale,
            )?))
        }
        _ => Err(ModelError::Type),
    }
}
