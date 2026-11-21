//! Private receipt journal/outbox; contract lifecycle ownership stays in live::persist_events.
use crate::StoreError;
use cantongrid_core::{config::Plan, exercise::Exercise, lifecycle::Batch};
use cantongrid_postgres::postgres::Transaction;
use std::collections::BTreeSet;

/// Validate one complete forward transaction, or a snapshot delivery without past exercises.
pub(crate) fn validate(
    plan: &Plan,
    batch: &Batch,
    receipts: Option<&Vec<Exercise>>,
) -> Result<Option<serde_json::Value>, StoreError> {
    if plan.includes_exercises() != receipts.is_some() {
        return Err(StoreError::InvalidBatch);
    }
    let Some(receipts) = receipts else {
        return Ok(None);
    };
    if receipts.len() + batch.events.len() > 64 {
        return Err(StoreError::Limit);
    }
    let mut offsets = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut transactions = BTreeSet::new();
    let mut snapshots = BTreeSet::new();
    for event in &batch.events {
        let origin = event.origin.as_ref().ok_or(StoreError::InvalidBatch)?;
        snapshots.insert(origin.snapshot_observation);
        if !origin.snapshot_observation {
            offsets.insert(origin.offset);
        }
    }
    for receipt in receipts {
        if !receipt.validate(plan) || !keys.insert((receipt.offset, receipt.node_id)) {
            return Err(StoreError::InvalidBatch);
        }
        offsets.insert(receipt.offset);
        transactions.insert(&receipt.transaction_id);
    }
    if offsets.len() > 1
        || transactions.len() > 1
        || snapshots.len() > 1
        || (snapshots.contains(&true) && !receipts.is_empty())
    {
        return Err(StoreError::InvalidBatch);
    }
    serde_json::to_value(receipts)
        .map(Some)
        .map_err(|_| StoreError::InvalidBatch)
}

/// The caller holds the ordinary stream writer lock and inserts its batch first.
/// Unique receipt keys deliberately reject identity reuse across deliveries, rolling all work back.
pub(crate) fn persist(
    tx: &mut Transaction<'_>,
    stream: &str,
    revision: i64,
    receipts: &serde_json::Value,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO cantongrid.exercise_pages VALUES($1,$2,$3)",
        &[&stream, &revision, &receipts],
    )?;
    for receipt in receipts.as_array().ok_or(StoreError::InvalidBatch)? {
        let offset = receipt["offset"].as_i64().ok_or(StoreError::InvalidBatch)?;
        let node = i32::try_from(
            receipt["node_id"]
                .as_i64()
                .ok_or(StoreError::InvalidBatch)?,
        )
        .map_err(|_| StoreError::InvalidBatch)?;
        tx.execute(
            "INSERT INTO cantongrid.exercise_events VALUES($1,$2,$3,$4,$5)",
            &[&stream, &offset, &node, &revision, &receipt],
        )?;
        tx.execute("INSERT INTO cantongrid.exercise_outbox(stream_key,source_offset,node_id) VALUES($1,$2,$3)", &[&stream,&offset,&node])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantongrid_core::{
        authorization::{MAX_SOURCE_LEASE, SourceLease},
        config::{SourceContext, TemplateId},
        exercise::ExercisePage,
        lifecycle::Origin,
        visibility::PartyId,
    };
    use std::time::{Duration, Instant};

    /// Synthetic receipt authority is independent from the previously indexed contract state.
    fn receipt(plan: &Plan) -> Exercise {
        Exercise {
            version: 1,
            offset: 10,
            node_id: 0,
            transaction_id: "transaction".into(),
            contract: "unindexed-parent".into(),
            template: plan.templates().next().unwrap().clone(),
            choice: "Create".into(),
            consuming: true,
            argument: r#"{"exact":"9223372036854775807"}"#.into(),
            result: "null".into(),
            controllers: ["operator".into()].into(),
            witnesses: ["A".into()].into(),
            ledger_at_micros: None,
            last_descendant_node_id: -1,
        }
    }

    #[test]
    fn profile_bounds_and_transaction_integrity_fail_closed() {
        let plan = Plan::pqs(
            SourceContext::new("test".into(), "synthetic".into(), 1).unwrap(),
            [PartyId::new("A").unwrap()].into(),
            [TemplateId {
                package_id: "0".repeat(64),
                module: "M".into(),
                entity: "T".into(),
            }]
            .into(),
        )
        .unwrap();
        let batch = Batch {
            context: plan.context().clone(),
            position: "page".into(),
            events: vec![],
        };
        assert!(validate(&plan, &batch, Some(&vec![])).is_err());
        let plan = plan.with_exercises().unwrap();
        let one = receipt(&plan);
        assert!(validate(&plan, &batch, Some(&vec![one.clone()])).is_ok());
        assert!(validate(&plan, &batch, None).is_err());
        assert!(validate(&plan, &batch, Some(&vec![one.clone(); 65])).is_err());
        assert!(validate(&plan, &batch, Some(&vec![one.clone(), one.clone()])).is_err());
        let mut other = one.clone();
        other.offset += 1;
        assert!(validate(&plan, &batch, Some(&vec![one, other])).is_err());
    }

    #[test]
    #[ignore = "requires owned PostgreSQL; run serially"]
    fn exercise_journal_replay_conflicts_and_contract_effects_are_atomic() {
        use crate::model_projection::tests::{PAYLOAD, Scenario};
        let mut s = Scenario::visibility();
        let legacy = s.plans[0].clone();
        s.plans[0] = s.plans[0].clone().with_exercises().unwrap();
        let plan = &s.plans[0];
        let mut store = s.open();
        let lease = s.lease();
        let initial = ExercisePage {
            batch: Batch {
                context: plan.context().clone(),
                position: "snapshot".into(),
                events: vec![],
            },
            exercises: vec![],
        };
        store
            .apply_exercise_page(plan, &initial, &lease, None)
            .unwrap();
        assert_eq!(store.checkpoint(&legacy), Err(StoreError::PlanChanged));
        // Missing delivery evidence is corruption, not permission to continue past the head.
        let key = crate::journal::stream_key(plan).0;
        let mut tx = store.client.transaction().unwrap();
        tx.execute(
            "DELETE FROM cantongrid.exercise_pages WHERE stream_key=$1",
            &[&key],
        )
        .unwrap();
        assert_eq!(
            crate::live::prefix(&mut tx, plan),
            Err(StoreError::CorruptJournal)
        );
        tx.rollback().unwrap();
        store
            .client
            .execute(
                "DELETE FROM cantongrid.exercise_pages WHERE stream_key=$1",
                &[&key],
            )
            .unwrap();
        assert_eq!(
            store.apply_exercise_page(plan, &initial, &lease, Some("snapshot")),
            Err(StoreError::CorruptJournal)
        );
        store
            .client
            .execute(
                "INSERT INTO cantongrid.exercise_pages VALUES($1,1,'[]'::jsonb)",
                &[&key],
            )
            .unwrap();
        s.bind(1, None).unwrap();
        let mut batch = s.create("one", "child", PAYLOAD);
        batch.events[0].origin = Some(Origin {
            offset: 10,
            snapshot_observation: false,
        });
        let page = ExercisePage {
            batch,
            exercises: vec![receipt(plan)],
        };
        assert!(
            store
                .apply_exercise_page(plan, &page, &lease, Some("snapshot"))
                .unwrap()
        );
        assert!(
            !s.open()
                .apply_exercise_page(plan, &page, &lease, Some("one"))
                .unwrap()
        );
        assert_eq!(
            store.apply_exercise_page(plan, &page, &lease, Some("snapshot")),
            Err(StoreError::Busy)
        );
        assert_eq!(s.count(0, plan), 1); // A consuming receipt does not archive its unrelated child.
        let key = crate::journal::stream_key(plan).0;
        let before: i64 = store
            .client
            .query_one(
                "SELECT count(*) FROM cantongrid.exercise_outbox WHERE stream_key=$1",
                &[&key],
            )
            .unwrap()
            .get(0);
        let mut conflicting = page.clone();
        conflicting.batch = s.create("two", "staged", PAYLOAD);
        conflicting.batch.events[0].origin = Some(Origin {
            offset: 10,
            snapshot_observation: false,
        });
        conflicting.exercises[0].argument = "{}".into();
        assert!(
            store
                .apply_exercise_page(plan, &conflicting, &lease, Some("one"))
                .is_err()
        );
        assert_eq!(store.checkpoint(plan).unwrap().as_deref(), Some("one"));
        assert_eq!(s.count(0, plan), 1);
        assert_eq!(
            store
                .client
                .query_one(
                    "SELECT count(*) FROM cantongrid.exercise_outbox WHERE stream_key=$1",
                    &[&key]
                )
                .unwrap()
                .get::<_, i64>(0),
            before
        );
        let mut revoked = lease;
        revoked.invalidate();
        assert_eq!(
            store.apply_exercise_page(plan, &page, &revoked, Some("one")),
            Err(StoreError::Authorization)
        );

        // Force expiry after staging lifecycle/model/receipt/outbox writes. Final lease check
        // must roll back all of them; the delay exists only in this disposable test database.
        store.client.batch_execute("CREATE FUNCTION cantongrid.exercise_delay_test() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_sleep(1); RETURN NEW; END$$; CREATE TRIGGER exercise_delay_test BEFORE INSERT ON cantongrid.exercise_outbox FOR EACH ROW EXECUTE FUNCTION cantongrid.exercise_delay_test();").unwrap();
        let now = Instant::now();
        let expiring = SourceLease::from_verified(
            plan.context().binding(),
            [PartyId::new("A").unwrap()],
            now,
            MAX_SOURCE_LEASE,
            now + Duration::from_millis(500),
        )
        .unwrap();
        let mut late = conflicting;
        late.exercises[0].node_id = 2;
        let result = store.apply_exercise_page(plan, &late, &expiring, Some("one"));
        store.client.batch_execute("DROP TRIGGER exercise_delay_test ON cantongrid.exercise_outbox; DROP FUNCTION cantongrid.exercise_delay_test();").unwrap();
        assert_eq!(result, Err(StoreError::Authorization));
        assert_eq!(store.checkpoint(plan).unwrap().as_deref(), Some("one"));
        assert_eq!(s.count(0, plan), 1);
    }
}
