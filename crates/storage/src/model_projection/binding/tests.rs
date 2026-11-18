//! Activation controls operate on a disposable destination and mock source leases only.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, create, lease};
use cantongrid_core::{config::SourceContext, visibility::PartyId};

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn promotion_is_compare_and_replace_and_failed_backfill_preserves_old_rows() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let mut next = s.models[0].clone();
    next.revision = 2;
    // This path is absent: evaluation fails after cleanup, testing transaction rollback.
    if let cantongrid_models::Expression::Field { path, .. } =
        next.fields.get_mut("amount").unwrap()
    {
        *path = vec!["missing".into()];
    }
    s.install(next);
    assert_eq!(s.bind(2, Some(1)), Err(StoreError::ModelProjection));
    assert_eq!(s.count(0, &s.plans[0]), 1);
    assert_eq!(s.count(1, &s.plans[0]), 0);
    let mut next = s.models[0].clone();
    next.revision = 3;
    s.install(next);
    assert_eq!(s.bind(3, None), Err(StoreError::ModelBinding));
    assert_eq!(s.bind(3, Some(2)), Err(StoreError::ModelBinding));
    assert!(s.bind(3, Some(1)).unwrap().changed);
    assert_eq!(s.count(0, &s.plans[0]), 0);
    assert_eq!(s.count(2, &s.plans[0]), 1);
    assert!(!s.bind(3, Some(1)).unwrap().changed); // Retry after uncertain commit.
    assert!(s.bind(1, Some(3)).unwrap().changed); // Explicit rollback rebuilds current state.
    assert_eq!(s.count(0, &s.plans[0]), 1);
    assert_eq!(s.count(2, &s.plans[0]), 0);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn wrong_source_selection_generation_and_lease_cannot_bind_or_refresh() {
    let s = Scenario::visibility();
    assert_eq!(s.bind(1, None), Err(StoreError::ModelBinding)); // No indexed prefix.
    s.seed();
    let plan = &s.plans[0];
    let mut store = s.open();
    let wrong = plan.clone().with_source_binding("b".repeat(64));
    assert_eq!(
        store.bind_model(&wrong, &s.lease(), &s.namespace, &s.models[0].name, 1, None),
        Err(StoreError::PlanChanged)
    );
    let unbound = Plan::pqs(
        plan.context().clone(),
        plan.source_parties().cloned().collect(),
        plan.templates().cloned().collect(),
    )
    .unwrap();
    assert_eq!(
        store.bind_model(
            &unbound,
            &s.lease(),
            &s.namespace,
            &s.models[0].name,
            1,
            None
        ),
        Err(StoreError::ModelBinding)
    );
    let other = Plan::pqs(
        SourceContext::new("other".into(), "synthetic".into(), 2).unwrap(),
        [PartyId::new("A").unwrap()].into(),
        plan.templates().cloned().collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    assert_eq!(
        store.bind_model(&other, &s.lease(), &s.namespace, &s.models[0].name, 1, None),
        Err(StoreError::Authorization)
    );
    for parties in [vec!["B"], vec!["A", "B"]] {
        let wrong = lease(plan, parties);
        assert_eq!(
            store.bind_model(plan, &wrong, &s.namespace, &s.models[0].name, 1, None),
            Err(StoreError::Authorization)
        );
        assert_eq!(
            store.observe_authorized(plan, &wrong),
            Err(StoreError::Authorization)
        );
    }
    let mut revoked = s.lease();
    revoked.invalidate();
    assert_eq!(
        store.bind_model(plan, &revoked, &s.namespace, &s.models[0].name, 1, None),
        Err(StoreError::Authorization)
    );
    assert_eq!(s.count(0, plan), 0);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn one_source_lock_serializes_binding_ingestion_and_idle_observation() {
    let s = Scenario::visibility();
    s.seed();
    let mut blocker = s.open();
    let mut tx = blocker.client.transaction().unwrap();
    lock(&mut tx, &s.plans[0]).unwrap();
    assert_eq!(s.bind(1, None), Err(StoreError::Busy));
    let mut store = s.open();
    assert_eq!(
        store.observe_authorized(&s.plans[0], &s.lease()),
        Err(StoreError::Busy)
    );
    assert_eq!(
        store.apply_authorized(
            &s.plans[0],
            &s.create("busy", "second", PAYLOAD),
            &s.lease()
        ),
        Err(StoreError::Busy)
    );
    tx.rollback().unwrap();
    assert!(s.bind(1, None).unwrap().changed);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn backfill_row_limit_and_sql_abort_leave_no_partial_activation() {
    let s = Scenario::visibility();
    s.seed();
    let mut store = s.open();
    let result = store.bind_model_checked(
        &s.plans[0],
        &s.lease(),
        &s.namespace,
        &s.models[0].name,
        1,
        None,
        |tx| {
            tx.batch_execute("SELECT 1/0")?;
            Ok(())
        },
    );
    assert!(matches!(result, Err(StoreError::Database(_))));
    assert_eq!(s.count(0, &s.plans[0]), 0);
    for n in 0..2 {
        let mut batch = s.create(&format!("page-{n}"), "unused", PAYLOAD);
        batch.events.clear();
        for i in 0..64 {
            batch.events.extend(
                s.create("unused", &format!("contract-{n}-{i}"), PAYLOAD)
                    .events,
            );
        }
        store
            .apply_authorized(&s.plans[0], &batch, &s.lease())
            .unwrap();
    }
    assert_eq!(s.bind(1, None), Err(StoreError::Limit)); // 129 active contracts.
    assert_eq!(s.count(0, &s.plans[0]), 0);
    assert_eq!(
        store.checkpoint(&s.plans[0]).unwrap().as_deref(),
        Some("page-1")
    );
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn shared_physical_table_keeps_streams_isolated() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let plan = Scenario::plan(&format!("{}_other", s.namespace), &s.models[0]);
    s.plans.push(plan.clone());
    let mut store = s.open();
    store
        .apply_authorized(
            &plan,
            &create(&plan, &s.models[0], "snapshot", "first", PAYLOAD),
            &lease(&plan, vec!["A"]),
        )
        .unwrap();
    store
        .bind_model(
            &plan,
            &lease(&plan, vec!["A"]),
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
    assert_eq!(s.count(0, &plan), 1);
    let mut next = s.models[0].clone();
    next.revision = 2;
    s.install(next);
    s.bind(2, Some(1)).unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 0);
    assert_eq!(s.count(0, &plan), 1);
    assert_eq!(s.count(1, &plan), 0);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn aliases_deduplicate_live_writes_and_preserve_referenced_old_versions() {
    let mut s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let alias = format!("{}_alias", s.namespace);
    s.aliases.push(alias.clone());
    let catalog = cantongrid_models::Catalog::parse(
        &serde_json::json!({"version":1,"models":[s.models[0]]}).to_string(),
    )
    .unwrap();
    let mut store = s.open();
    store.register_models(&alias, &catalog).unwrap();
    store.install_model(&alias, &s.models[0].name, 1).unwrap();
    store
        .bind_model(&s.plans[0], &s.lease(), &alias, &s.models[0].name, 1, None)
        .unwrap();
    store
        .apply_authorized(
            &s.plans[0],
            &s.create("future", "second", PAYLOAD),
            &s.lease(),
        )
        .unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 2); // One INSERT despite two aliases.
    let mut next = s.models[0].clone();
    next.revision = 2;
    s.install(next);
    s.bind(2, Some(1)).unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 2); // The other namespace still uses revision 1.
    store
        .register_models(
            &alias,
            &cantongrid_models::Catalog::parse(
                &serde_json::json!({"version":1,"models":[s.models[1]]}).to_string(),
            )
            .unwrap(),
        )
        .unwrap();
    store.install_model(&alias, &s.models[0].name, 2).unwrap();
    store
        .bind_model(
            &s.plans[0],
            &s.lease(),
            &alias,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 0); // Last old reference removed atomically.
    assert_eq!(s.count(1, &s.plans[0]), 2);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn model_count_and_shared_work_budget_bound_live_processing() {
    let mut s = Scenario::visibility();
    let mut store = s.open();
    // A real acknowledged empty page permits binding without fabricating a stream row.
    let mut empty = s.create("empty", "unused", PAYLOAD);
    empty.events.clear();
    store
        .apply_authorized(&s.plans[0], &empty, &s.lease())
        .unwrap();
    s.bind(1, None).unwrap();
    for i in 1..=16 {
        let mut model = s.models[0].clone();
        model.name = format!("{}_m{i}", s.namespace);
        s.install(model.clone());
        let result = store.bind_model(&s.plans[0], &s.lease(), &s.namespace, &model.name, 1, None);
        if i == 16 {
            assert_eq!(result, Err(StoreError::Limit));
        } else {
            assert!(result.unwrap().changed);
        }
    }
    // Eight 40 KiB labels fit the canonical 1 MiB page. Evaluating/input+output across
    // sixteen models exceeds the separate 8 MiB projection budget and must roll it all back.
    let mut payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
    payload["label"] = "x".repeat(40000).into();
    let mut batch = empty;
    batch.position = "budget".into();
    for i in 0..8 {
        batch.events.extend(
            s.create("unused", &format!("big-{i}"), &payload.to_string())
                .events,
        );
    }
    let before = store.stats(&s.plans[0]).unwrap();
    assert_eq!(
        store.apply_authorized(&s.plans[0], &batch, &s.lease()),
        Err(StoreError::Limit)
    );
    assert_eq!(store.stats(&s.plans[0]).unwrap(), before);
    for i in 0..s.models.len() {
        assert_eq!(s.count(i, &s.plans[0]), 0);
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn lease_expiring_after_backfill_rolls_back_binding_and_rows() {
    use cantongrid_core::authorization::{MAX_SOURCE_LEASE, SourceLease};
    use std::time::{Duration, Instant};
    let s = Scenario::visibility();
    s.seed();
    let now = Instant::now();
    let deadline = now + Duration::from_secs(2);
    let short = SourceLease::from_verified(
        s.plans[0].context().binding(),
        s.plans[0].source_parties().cloned(),
        now,
        MAX_SOURCE_LEASE,
        deadline,
    )
    .unwrap();
    let mut reached = false;
    let result = s.open().bind_model_checked(
        &s.plans[0],
        &short,
        &s.namespace,
        &s.models[0].name,
        1,
        None,
        |_| {
            reached = true;
            // Wait only inside this failure injection; production never sleeps while holding a lease.
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            Ok(())
        },
    );
    assert!(reached);
    assert_eq!(result, Err(StoreError::Authorization));
    assert_eq!(s.count(0, &s.plans[0]), 0);
    assert!(s.bind(1, None).unwrap().changed);
}
