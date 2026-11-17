//! Meaningful controls for determinism, exact arithmetic, invalid definitions and generated storage.
use super::*;
use cantongrid_core::value::{Decimal, Value};
use serde_json::{Value as Json, json};

fn visibility() -> Catalog {
    Catalog::parse(include_str!("../../../examples/models/visibility.json")).unwrap()
}
fn payload() -> Json {
    json!({"label":"test","owner":"A","amount":"12345678901234567890.1234567891","sequence":"9223372036854775807","note":null,"state":"Ready"})
}
fn literal(value: Value) -> Expression {
    Expression::Literal { value }
}
fn definition() -> Json {
    serde_json::from_str(include_str!("../../../examples/models/visibility.json")).unwrap()
}

#[test]
fn outcomia_and_unrelated_definitions_use_the_same_exact_evaluator() {
    let outcomia = Catalog::parse(include_str!("../../../examples/models/outcomia.json")).unwrap();
    let input = json!({"marketId":"7","protocolName":"test","status":"Closed","terms":{"question":"Test question","priceTick":"0.0000000001","quantityLot":"9223372036854775807"}});
    let row = outcomia.models()[0]
        .evaluate(&input.to_string())
        .unwrap()
        .unwrap();
    assert_eq!(
        row["priceTickBps"],
        Value::Numeric(Decimal {
            scale: 10,
            decimal: "0.0000010000".into()
        })
    );
    assert_eq!(row["quantityLot"], Value::Int64(i64::MAX.to_string()));
    let catalog = visibility();
    let model = &catalog.models()[0];
    let row = model.evaluate(&payload().to_string()).unwrap().unwrap();
    assert_eq!(
        row["amount"],
        Value::Numeric(Decimal {
            scale: 10,
            decimal: "12345678901234567890.1234567891".into()
        })
    );
    assert_eq!(row["sequence"], Value::Int64(i64::MAX.to_string()));
    assert_eq!(row["note"], Value::Optional(None));
}

#[test]
fn fingerprints_and_ddl_are_deterministic_and_names_cannot_inject_sql() {
    let first = visibility();
    let text = serde_json::to_string(&definition()).unwrap();
    let second = Catalog::parse(&text).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    let plan = first.models()[0].sql_plan().unwrap();
    assert!(plan.table.len() < 64);
    assert!(
        plan.create_sql
            .contains("\"amount\" numeric(38,10) NOT NULL")
    );
    assert!(plan.create_sql.contains("\"sequence\" bigint NOT NULL"));
    assert!(plan.create_sql.contains("\"note\" text,"));
    assert!(plan.create_sql.contains("REFERENCES cantongrid.contracts"));
    assert_eq!(
        plan.create_sql,
        second.models()[0].sql_plan().unwrap().create_sql
    );
    let mut changed = definition();
    changed["models"][0]["revision"] = 2.into();
    assert_ne!(
        first.fingerprint(),
        Catalog::parse(&changed.to_string()).unwrap().fingerprint()
    );
    for bad in [
        "bad\"; DROP SCHEMA public",
        "activeContracts",
        "_cg_stream_key",
        "__schema",
    ] {
        let mut input = definition();
        input["models"][0]["name"] = bad.into();
        assert!(Catalog::parse(&input.to_string()).is_err());
    }
}

#[test]
fn filters_optional_paths_and_type_errors_do_not_become_zero_or_empty_success() {
    let catalog = visibility();
    let model = &catalog.models()[0];
    let mut input = payload();
    input.as_object_mut().unwrap().remove("note");
    assert_eq!(
        model.evaluate(&input.to_string()).unwrap().unwrap()["note"],
        Value::Optional(None)
    );
    input["state"] = "Archived".into();
    assert!(model.evaluate(&input.to_string()).unwrap().is_none());
    input["state"] = "Ready".into();
    input["note"] = true.into();
    assert_eq!(model.evaluate(&input.to_string()), Err(ModelError::Type));
    input = payload();
    input.as_object_mut().unwrap().remove("owner");
    assert_eq!(model.evaluate(&input.to_string()), Err(ModelError::Missing));
    input = payload();
    input["sequence"] = json!(9223372036854775807_i64);
    assert_eq!(model.evaluate(&input.to_string()), Err(ModelError::Type));
    input = payload();
    input["amount"] = "1e10".into();
    assert_eq!(model.evaluate(&input.to_string()), Err(ModelError::Type));
    assert_eq!(
        model.evaluate(r#"{"amount":1.234567890123456789}"#),
        Err(ModelError::Type)
    );
}

#[test]
fn decimal_normalization_is_exact_and_never_rounds() {
    assert_eq!(
        crate::arithmetic::decimal("1.2", 3).unwrap().decimal,
        "1.200"
    );
    assert_eq!(
        crate::arithmetic::decimal("-0.0", 3).unwrap().decimal,
        "0.000"
    );
    for input in ["1.2345", "01.2", "+1.2", "1e2", ".2", "1.", "--1.2"] {
        assert!(crate::arithmetic::decimal(input, 3).is_err());
    }
    assert!(crate::arithmetic::decimal("1", 38).is_err());
}

#[test]
fn checked_arithmetic_rejects_integer_decimal_overflow_and_unsupported_types() {
    let calc = |op, left, right| crate::arithmetic::calculate(op, left, right);
    use crate::arithmetic::Operation;
    assert_eq!(
        calc(
            Operation::Add,
            Value::Int64(i64::MAX.to_string()),
            Value::Int64("1".into())
        ),
        Err(ModelError::Overflow)
    );
    assert_eq!(
        calc(
            Operation::Subtract,
            Value::Int64(i64::MIN.to_string()),
            Value::Int64("1".into())
        ),
        Err(ModelError::Overflow)
    );
    let maximum = Value::Numeric(Decimal {
        scale: 0,
        decimal: "9".repeat(38),
    });
    assert_eq!(
        calc(
            Operation::Add,
            maximum.clone(),
            Value::Numeric(Decimal {
                scale: 0,
                decimal: "1".into()
            })
        ),
        Err(ModelError::Overflow)
    );
    assert_eq!(
        calc(
            Operation::Multiply,
            maximum,
            Value::Int64(i64::MAX.to_string())
        ),
        Err(ModelError::Overflow)
    );
    let incompatible = Expression::Add {
        left: Box::new(literal(Value::Int64("1".into()))),
        right: Box::new(literal(Value::Text("SECRET_SENTINEL".into()))),
    };
    assert_eq!(incompatible.output_type(), Err(ModelError::Type));
    assert!(!format!("{incompatible:?}").contains("SECRET_SENTINEL"));
}

#[test]
fn unknown_duplicate_unbounded_and_ambiguous_definitions_fail_closed() {
    let duplicate = r#"{"version":1,"version":1,"models":[]}"#;
    assert!(Catalog::parse(duplicate).is_err());
    let mut input = definition();
    input["secret"] = "SECRET_SENTINEL".into();
    let error = Catalog::parse(&input.to_string()).unwrap_err();
    assert!(!format!("{error:?}: {error}").contains("SECRET_SENTINEL"));
    input = definition();
    input["models"][0]["fields"]["amount"]["as"]["scale"] = 38.into();
    assert!(Catalog::parse(&input.to_string()).is_err());
    input = definition();
    input["models"][0]["fields"]["note"]["as"] =
        json!({"type":"optional","item":{"type":"optional","item":{"type":"text"}}});
    assert!(Catalog::parse(&input.to_string()).is_err());
    input = definition();
    input["models"][0]["fields"]["amount"]["path"] = json!(["amount; DROP TABLE"]);
    assert!(Catalog::parse(&input.to_string()).is_err());
    let mut expression = literal(Value::Int64("1".into()));
    for _ in 0..18 {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(literal(Value::Int64("1".into()))),
        };
    }
    assert_eq!(expression.output_type(), Err(ModelError::Limit));
    assert_eq!(
        Catalog::parse(&" ".repeat(65537)).unwrap_err(),
        ModelError::Limit
    );
}

#[test]
#[ignore = "requires explicitly initialized owned PostgreSQL destination"]
fn generated_schema_round_trips_exact_values_and_keeps_nullable_fields() {
    let catalog = visibility();
    let model = &catalog.models()[0];
    let plan = model.sql_plan().unwrap();
    let projected = model.evaluate(&payload().to_string()).unwrap().unwrap();
    let stream = format!("model-plan-test-{}", std::process::id());
    let contract = "synthetic-contract";
    let row = model.row_json(&stream, contract, &projected).unwrap();
    let mut client = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    let mut tx = client.transaction().unwrap();
    tx.batch_execute("CREATE SCHEMA IF NOT EXISTS cantongrid_models")
        .unwrap();
    tx.execute(
        "INSERT INTO cantongrid.streams(stream_key,definition) VALUES($1,'{}')",
        &[&stream],
    )
    .unwrap();
    tx.execute("INSERT INTO cantongrid.contracts(stream_key,contract_id,template,payload,create_event_id,witnesses,active) VALUES($1,$2,'{}','{}','synthetic-create',ARRAY['A'],true)",&[&stream,&contract]).unwrap();
    tx.batch_execute(&plan.create_sql).unwrap();
    let insert = format!(
        "INSERT INTO cantongrid_models.\"{}\" SELECT * FROM jsonb_populate_record(NULL::cantongrid_models.\"{}\",$1)",
        plan.table, plan.table
    );
    tx.execute(&insert, &[&row]).unwrap();
    let select = format!(
        "SELECT \"amount\"::text,\"sequence\"::text,\"note\" FROM cantongrid_models.\"{}\"",
        plan.table
    );
    let actual = tx.query_one(&select, &[]).unwrap();
    assert_eq!(
        actual.get::<_, String>(0),
        "12345678901234567890.1234567891"
    );
    assert_eq!(actual.get::<_, String>(1), i64::MAX.to_string());
    assert_eq!(actual.get::<_, Option<String>>(2), None);
    tx.rollback().unwrap();
}
