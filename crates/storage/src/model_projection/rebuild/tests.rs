//! Owned-destination tests use synthetic source leases; no Canton/Ledger writes occur.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario};
use cantongrid_core::{
    lifecycle::Action,
    visibility::{CallerGrants, PartyId},
};

fn step(
    s: &Scenario,
    id: &str,
    action: RebuildAction,
    limit: usize,
) -> Result<RebuildStatus, StoreError> {
    s.open()
        .model_rebuild(&s.plans[0], &s.lease(), id, action, limit)
}
fn until(s: &Scenario, id: &str, phase: &str) -> RebuildStatus {
    for _ in 0..100 {
        let result = step(s, id, RebuildAction::Step, 32).unwrap();
        if result.phase == phase {
            return result;
        }
    }
    panic!("bounded test job did not reach {phase}");
}
fn revision(s: &mut Scenario) {
    let mut model = s.models[0].clone();
    model.revision = 2;
    s.install(model);
}
fn add(s: &Scenario, position: &str, contract: &str) {
    s.open()
        .apply_authorized(
            &s.plans[0],
            &s.create(position, contract, PAYLOAD),
            &s.lease(),
        )
        .unwrap();
}
fn archive(s: &Scenario, position: &str, contract: &str) {
    let mut batch = s.create(position, contract, PAYLOAD);
    batch.events[0].id = format!("archive-{contract}");
    batch.events[0].witnesses.clear();
    batch.events[0].action = Action::ArchiveByPriorContract;
    s.open()
        .apply_authorized(&s.plans[0], &batch, &s.lease())
        .unwrap();
}
fn caller(s: &Scenario) -> CallerGrants {
    CallerGrants::from_verified(s.plans[0].context().binding(), [PartyId::new("A").unwrap()])
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn large_rebuild_resumes_catches_behind_cursor_changes_and_activates_without_copying() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    revision(&mut s);
    // Grow a live model beyond the legacy atomic backfill/cleanup limit.
    for chunk in 0..5 {
        let mut batch = s.create(&format!("batch-{chunk}"), "unused", PAYLOAD);
        batch.events.clear();
        for index in 0..52 {
            batch.events.extend(
                s.create("unused", &format!("row-{:03}", chunk * 52 + index), PAYLOAD)
                    .events,
            );
        }
        s.open()
            .apply_authorized(&s.plans[0], &batch, &s.lease())
            .unwrap();
    }
    assert_eq!(s.count(0, &s.plans[0]), 261);
    assert!(matches!(s.bind(2, Some(1)), Err(StoreError::Limit)));
    let started = s
        .open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "v2",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    assert_eq!(started.phase, "cleaning");
    assert_eq!(
        s.open()
            .start_model_rebuild(
                &s.plans[0],
                &s.lease(),
                "v2",
                &s.namespace,
                &s.models[0].name,
                2,
                Some(1)
            )
            .unwrap()
            .phase,
        "cleaning"
    );
    assert!(matches!(s.bind(1, None), Err(StoreError::ModelRebuild)));
    step(&s, "v2", RebuildAction::Step, 64).unwrap(); // Empty target -> backfill.
    let first = step(&s, "v2", RebuildAction::Step, 64).unwrap();
    assert_eq!(first.scanned, "64");
    assert_eq!(
        s.open()
            .start_model_rebuild(
                &s.plans[0],
                &s.lease(),
                "v2",
                &s.namespace,
                &s.models[0].name,
                2,
                Some(1)
            )
            .unwrap()
            .scanned,
        "64"
    );
    assert!(matches!(
        s.open().start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "other",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1)
        ),
        Err(StoreError::ModelRebuild)
    ));

    let old = s
        .open()
        .bound_models(&s.plans[0], &caller(&s), &s.lease())
        .unwrap()
        .remove(0);
    assert_eq!(old.model.revision, 1);
    // Rows have been physically staged but neither discovery nor a fabricated binding exposes them.
    let staged = crate::BoundModel {
        namespace: s.namespace.clone(),
        model: s.models[1].clone(),
        fingerprint: s.models[1].fingerprint().unwrap(),
    };
    assert!(matches!(
        s.open()
            .read_model(&s.plans[0], &caller(&s), &s.lease(), &staged, None, 1),
        Err(StoreError::ModelBinding)
    ));
    add(&s, "late-create", "aaa-behind-cursor");
    archive(&s, "remove-scanned", "first");
    archive(&s, "remove-unscanned", "row-259");
    add(&s, "short-lived-create", "aaa-short-lived");
    archive(&s, "short-lived-archive", "aaa-short-lived");
    until(&s, "v2", "ready");
    // A change after readiness invalidates activation, even though the phase label still says ready.
    add(&s, "after-ready", "aaa-after-ready");
    assert!(
        step(&s, "v2", RebuildAction::Status, 32)
            .unwrap()
            .has_pending
    );
    assert!(matches!(
        step(&s, "v2", RebuildAction::Activate, 32),
        Err(StoreError::ModelRebuild)
    ));
    let ready = until(&s, "v2", "ready");
    assert_eq!(ready.projected_rows, "261");
    assert_eq!(
        step(&s, "v2", RebuildAction::Activate, 32).unwrap().phase,
        "cleanup"
    );
    assert_eq!(
        s.count(0, &s.plans[0]),
        261,
        "activation must not perform an unbounded old-row delete"
    );
    assert_eq!(
        step(&s, "v2", RebuildAction::Activate, 32).unwrap().phase,
        "cleanup"
    );
    let new = s
        .open()
        .bound_models(&s.plans[0], &caller(&s), &s.lease())
        .unwrap()
        .remove(0);
    assert_eq!(new.model.revision, 2);
    assert_eq!(
        s.open()
            .read_model(&s.plans[0], &caller(&s), &s.lease(), &new, None, 1)
            .unwrap()
            .visible_total,
        "261"
    );
    step(&s, "v2", RebuildAction::Step, 32).unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 229);
    until(&s, "v2", "complete");
    assert_eq!(s.count(0, &s.plans[0]), 0);
    assert_eq!(s.count(1, &s.plans[0]), 261);
    add(&s, "after-activation", "new-live-row");
    assert_eq!(s.count(1, &s.plans[0]), 262);
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn cancellation_cleans_only_staging_and_source_authority_is_checked_on_each_restart() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    revision(&mut s);
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "cancel-me",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    step(&s, "cancel-me", RebuildAction::Step, 1).unwrap();
    step(&s, "cancel-me", RebuildAction::Step, 1).unwrap();
    assert_eq!(s.count(1, &s.plans[0]), 1);
    let wrong = crate::model_projection::tests::lease(&s.plans[0], vec!["B"]);
    assert!(matches!(
        s.open()
            .model_rebuild(&s.plans[0], &wrong, "cancel-me", RebuildAction::Step, 1),
        Err(StoreError::Authorization)
    ));
    assert_eq!(s.count(1, &s.plans[0]), 1);
    step(&s, "cancel-me", RebuildAction::Cancel, 1).unwrap();
    until(&s, "cancel-me", "cancelled");
    assert_eq!(s.count(1, &s.plans[0]), 0);
    assert_eq!(s.count(0, &s.plans[0]), 1);
    assert!(matches!(
        step(&s, "cancel-me", RebuildAction::Activate, 1),
        Err(StoreError::ModelRebuild)
    ));
    // A new identity can reserve the same revision after all cancellation cleanup committed.
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "retry",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn staged_expression_failure_preserves_page_progress_and_live_ingestion() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let mut definition = serde_json::to_value(&s.models[0]).unwrap();
    definition["revision"] = 2.into();
    definition["fields"]["amount"]["as"]["scale"] = 0.into();
    s.install(serde_json::from_value(definition).unwrap());
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "bad",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    step(&s, "bad", RebuildAction::Step, 1).unwrap();
    assert!(matches!(
        step(&s, "bad", RebuildAction::Step, 1),
        Err(StoreError::ModelProjection)
    ));
    let status = step(&s, "bad", RebuildAction::Status, 1).unwrap();
    assert_eq!(status.scanned, "0");
    assert_eq!(status.projected_rows, "0");
    add(&s, "healthy-live", "healthy-live");
    assert_eq!(s.count(0, &s.plans[0]), 2);
    assert!(
        step(&s, "bad", RebuildAction::Status, 1)
            .unwrap()
            .has_pending
    );
    step(&s, "bad", RebuildAction::Cancel, 1).unwrap();
    until(&s, "bad", "cancelled");
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn page_sql_abort_and_lease_expiry_after_writes_leave_no_partial_progress() {
    use cantongrid_core::authorization::MAX_SOURCE_LEASE;
    use std::time::{Duration, Instant};
    let mut s = Scenario::visibility();
    s.seed();
    revision(&mut s);
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "atomic",
            &s.namespace,
            &s.models[0].name,
            2,
            None,
        )
        .unwrap();
    step(&s, "atomic", RebuildAction::Step, 1).unwrap();
    let relation = s.relation(1);
    let failed = s.open().model_rebuild_checked(
        &s.plans[0],
        &s.lease(),
        "atomic",
        RebuildAction::Step,
        1,
        |tx| {
            assert_eq!(
                tx.query_one(&format!("SELECT count(*) FROM {relation}"), &[])?
                    .get::<_, i64>(0),
                1
            );
            tx.query_one("SELECT 1/0", &[])?;
            Ok(())
        },
    );
    assert!(matches!(failed, Err(StoreError::Database(_))));
    assert_eq!(s.count(1, &s.plans[0]), 0);
    assert_eq!(
        step(&s, "atomic", RebuildAction::Status, 1)
            .unwrap()
            .scanned,
        "0"
    );
    let lease = SourceLease::from_verified(
        s.plans[0].context().binding(),
        [PartyId::new("A").unwrap()],
        Instant::now(),
        MAX_SOURCE_LEASE,
        Instant::now() + Duration::from_millis(200),
    )
    .unwrap();
    let entered = std::cell::Cell::new(false);
    let failed = s.open().model_rebuild_checked(
        &s.plans[0],
        &lease,
        "atomic",
        RebuildAction::Step,
        1,
        |tx| {
            assert_eq!(
                tx.query_one(&format!("SELECT count(*) FROM {relation}"), &[])?
                    .get::<_, i64>(0),
                1
            );
            entered.set(true);
            std::thread::sleep(Duration::from_millis(220));
            Ok(())
        },
    );
    assert!(matches!(failed, Err(StoreError::Authorization)));
    assert!(
        entered.get(),
        "lease control must run after real page writes"
    );
    assert_eq!(s.count(1, &s.plans[0]), 0);
    assert_eq!(
        step(&s, "atomic", RebuildAction::Status, 1)
            .unwrap()
            .scanned,
        "0"
    );
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn retirement_preserves_other_aliases_and_a_second_stream_cannot_resume_the_job() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    revision(&mut s);
    let alias = format!("{}_alias", s.namespace);
    s.aliases.push(alias.clone());
    let catalog = cantongrid_models::Catalog::parse(
        &serde_json::json!({"version":1,"models":[s.models[0]]}).to_string(),
    )
    .unwrap();
    s.open().register_models(&alias, &catalog).unwrap();
    s.open()
        .install_model(&alias, &s.models[0].name, 1)
        .unwrap();
    s.open()
        .bind_model(&s.plans[0], &s.lease(), &alias, &s.models[0].name, 1, None)
        .unwrap();
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "aliases",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    let other = Scenario::plan(&format!("{}_other", s.namespace), &s.models[0]);
    let other_lease = crate::model_projection::tests::lease(&other, vec!["A"]);
    let mut batch = s.create("other-snapshot", "other", PAYLOAD);
    batch.context = other.context().clone();
    s.open()
        .apply_authorized(&other, &batch, &other_lease)
        .unwrap();
    s.plans.push(other.clone());
    assert!(matches!(
        s.open()
            .model_rebuild(&other, &other_lease, "aliases", RebuildAction::Step, 1),
        Err(StoreError::ModelRebuild)
    ));
    until(&s, "aliases", "ready");
    step(&s, "aliases", RebuildAction::Activate, 32).unwrap();
    until(&s, "aliases", "complete");
    assert_eq!(
        s.count(0, &s.plans[0]),
        1,
        "live alias must retain its original physical rows"
    );
    assert_eq!(s.count(1, &s.plans[0]), 1);
    assert_eq!(s.count(1, &other), 0);
}

/// First activation has no retired table; a failed metadata commit must leave no binding.
#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn initial_activation_is_atomic_and_uses_the_canonical_writer_lock() {
    let s = Scenario::visibility();
    s.seed();
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &s.lease(),
            "initial",
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
    until(&s, "initial", "ready");
    let mut blocker = s.open();
    let mut tx = blocker.client.transaction().unwrap();
    lock(&mut tx, &s.plans[0]).unwrap();
    for action in [
        RebuildAction::Step,
        RebuildAction::Activate,
        RebuildAction::Cancel,
    ] {
        assert!(matches!(
            step(&s, "initial", action, 1),
            Err(StoreError::Busy)
        ));
    }
    assert!(matches!(
        s.open().apply_authorized(
            &s.plans[0],
            &s.create("blocked", "blocked", PAYLOAD),
            &s.lease()
        ),
        Err(StoreError::Busy)
    ));
    tx.rollback().unwrap();
    let failed = s.open().model_rebuild_checked(
        &s.plans[0],
        &s.lease(),
        "initial",
        RebuildAction::Activate,
        1,
        |tx| {
            let key = stream_key(&s.plans[0]).0;
            assert_eq!(
                tx.query_one(
                    "SELECT count(*) FROM cantongrid.model_bindings WHERE stream_key=$1",
                    &[&key]
                )?
                .get::<_, i64>(0),
                1
            );
            tx.query_one("SELECT 1/0", &[])?;
            Ok(())
        },
    );
    assert!(matches!(failed, Err(StoreError::Database(_))));
    assert!(
        s.open()
            .bound_models(&s.plans[0], &caller(&s), &s.lease())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        step(&s, "initial", RebuildAction::Status, 1).unwrap().phase,
        "ready"
    );
    assert_eq!(
        step(&s, "initial", RebuildAction::Activate, 1)
            .unwrap()
            .phase,
        "complete"
    );
    assert_eq!(
        step(&s, "initial", RebuildAction::Activate, 1)
            .unwrap()
            .phase,
        "complete"
    );
    assert!(matches!(
        step(&s, "initial", RebuildAction::Cancel, 1),
        Err(StoreError::ModelRebuild)
    ));
    add(&s, "after-initial", "after-initial");
    assert_eq!(s.count(0, &s.plans[0]), 2);
}
