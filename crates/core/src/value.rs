//! Explicit fixture values: no untyped JSON numbers or floating point accounting.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Value {
    Text(String),
    /// Exact source JSON text; no Daml type inference or numeric operations are permitted.
    RawJson(String),
    Party(String),
    Bool(bool),
    Int64(String),
    Numeric(Decimal),
    Optional(Option<Box<Value>>),
    Enum(EnumValue),
    List(Vec<Value>),
    Record(BTreeMap<String, Value>),
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Decimal {
    pub scale: u8,
    pub decimal: String,
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumValue {
    pub type_id: String,
    pub constructor: String,
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Value([redacted])")
    }
}

impl Value {
    pub fn validate(&self) -> Result<(), ValueError> {
        self.validate_at(0)
    }

    fn validate_at(&self, depth: usize) -> Result<(), ValueError> {
        if depth > 64 {
            return Err(ValueError);
        }
        match self {
            Self::RawJson(text) => {
                serde_json::from_str::<Box<serde_json::value::RawValue>>(text)
                    .map_err(|_| ValueError)?;
            }
            Self::Party(value) => {
                if !crate::config::identifier(value) {
                    return Err(ValueError);
                }
            }
            Self::Int64(value) => {
                let parsed = value.parse::<i64>().map_err(|_| ValueError)?;
                if parsed.to_string() != *value {
                    return Err(ValueError);
                }
            }
            Self::Numeric(value) => value.validate()?,
            Self::Enum(value) => {
                if !crate::config::identifier(&value.type_id)
                    || !crate::config::identifier(&value.constructor)
                {
                    return Err(ValueError);
                }
            }
            Self::Optional(Some(value)) => value.validate_at(depth + 1)?,
            Self::List(values) => {
                for value in values {
                    value.validate_at(depth + 1)?;
                }
            }
            Self::Record(fields) => {
                for (name, value) in fields {
                    if !crate::config::identifier(name) {
                        return Err(ValueError);
                    }
                    value.validate_at(depth + 1)?;
                }
            }
            Self::Text(_) | Self::Bool(_) | Self::Optional(None) => {}
        }
        Ok(())
    }
}

impl Decimal {
    fn validate(&self) -> Result<(), ValueError> {
        if self.scale > 37 {
            return Err(ValueError);
        }
        let unsigned = self.decimal.strip_prefix('-').unwrap_or(&self.decimal);
        let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if integer.is_empty()
            || (integer.len() > 1 && integer.starts_with('0'))
            || !integer.bytes().all(|c| c.is_ascii_digit())
            || !fraction.bytes().all(|c| c.is_ascii_digit())
            || fraction.len() != self.scale as usize
            || (self.scale == 0 && unsigned.contains('.'))
            || integer.trim_start_matches('0').len() + self.scale as usize > 38
            || (self.decimal.starts_with('-') && unsigned.bytes().all(|c| c == b'0' || c == b'.'))
        {
            return Err(ValueError);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueError;
impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or unsupported typed value; check exact encoding, scale, precision and nesting")
    }
}
impl std::error::Error for ValueError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_values_round_trip_without_numeric_coercion() {
        let value = Value::Record(BTreeMap::from([
            (
                "amount".into(),
                Value::Numeric(Decimal {
                    scale: 10,
                    decimal: "12345678901234567890.1234567891".into(),
                }),
            ),
            ("sequence".into(), Value::Int64(i64::MAX.to_string())),
            ("minimum".into(), Value::Int64(i64::MIN.to_string())),
            ("note".into(), Value::Optional(None)),
        ]));
        value.validate().unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(encoded.contains("\"9223372036854775807\""));
        assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap(), value);
        assert!(!format!("{value:?}").contains("123456789"));
    }
    #[test]
    fn precision_scale_noncanonical_and_unsupported_values_are_rejected() {
        for decimal in [
            "1e2",
            "+1.00",
            "01.00",
            "-0.00",
            "1.0",
            "1.000",
            "9999999999999999999999999999999999999.00",
        ] {
            assert!(
                Value::Numeric(Decimal {
                    scale: 2,
                    decimal: decimal.into()
                })
                .validate()
                .is_err(),
                "{decimal}"
            );
        }
        for integer in [
            "9223372036854775808",
            "-9223372036854775809",
            "01",
            "+1",
            "-0",
            "1.0",
        ] {
            assert!(Value::Int64(integer.into()).validate().is_err());
        }
        assert!(
            serde_json::from_str::<Value>(r#"{"type":"int64","value":9223372036854775807}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<Value>(r#"{"type":"timestamp","value":"unknown"}"#).is_err()
        );
    }
}
