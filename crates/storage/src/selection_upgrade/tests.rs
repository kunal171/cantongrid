//! Owned PostgreSQL package-upgrade controls; no Canton transactions.

use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, create, lease};
use cantongrid_core::{
    lifecycle::Origin,
    visibility::{CallerGrants, PartyId},
};

/// A completed window cursor is the only safe point to widen an interval selection.
fn cursor(boundary: i64) -> String {
    serde_json::json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string()
}
/// Give the synthetic baseline its source evidence, as the real PQS reader does.
fn snapshot(s: &Scenario, plan: &Plan, contract: &str) -> Batch {
    let mut batch = create(plan, &s.models[1], &cursor(10), contract, PAYLOAD);
    batch.events[0].origin = Some(Origin {
        offset: 5,
        snapshot_observation: true,
    });
    batch
}
/// One scenario owns all rows/tables and keeps the old immutable model revision available for rollback.
fn scenario() -> (Scenario, Plan) {
    let mut s = Scenario::visibility();
    s.open()
        .apply_authorized(
            &s.plans[0],
            &s.create(&cursor(10), "old", PAYLOAD),
            &s.lease(),
        )
        .unwrap();
    s.bind(1, None).unwrap();
    let mut target = s.models[0].clone();
    target.revision = 2;
    target.template.package_id = "b".repeat(64);
    s.install(target);
    let after = Plan::pqs(
        s.plans[0].context().clone(),
        s.plans[0].source_parties().cloned().collect(),
        s.models.iter().map(|m| m.template.clone()).collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    (s, after)
}
/// Target revisions are installed explicitly before the read-only plan is produced.
fn plan(s: &Scenario, after: &Plan) -> UpgradeManifest {
    s.open()
        .plan_selection_upgrade(
            &s.plans[0],
            after,
            &s.lease(),
            &"c".repeat(64),
            &[(s.namespace.clone(), s.models[1].name.clone(), 2)],
        )
        .unwrap()
}
#[test]
fn partial_windows_and_snapshot_cursors_cannot_activate_upgrades() {
    assert_eq!(upgrade_boundary(&cursor(10)).unwrap(), 10);
    for field in ["snapshot", "through", "offset", "pk"] {
        let mut value: serde_json::Value = serde_json::from_str(&cursor(10)).unwrap();
        value[field] = if field == "snapshot" {
            true.into()
        } else {
            11.into()
        };
        assert!(upgrade_boundary(&value.to_string()).is_err());
    }
}
/// Baseline/model activation is atomic and repeatable after reopening. Subsequent mixed-package
/// creates/archives continue from the original checkpoint; stale workers cannot commit old selections.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn additive_upgrade_retains_checkpoint_history_and_supports_rebuild_rollback() {
    let (s, after) = scenario();
    let manifest = plan(&s, &after);
    let baseline = snapshot(&s, &after, "new");
    let result = s
        .open()
        .apply_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest, &baseline)
        .unwrap();
    assert!(result.changed);
    assert_eq!(s.open().checkpoint(&after).unwrap(), Some(cursor(10)));
    assert!(s.open().checkpoint(&s.plans[0]).is_err());
    assert!(
        !s.open()
            .apply_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest, &baseline)
            .unwrap()
            .changed
    );
    assert_eq!(s.count(1, &after), 1);
    let caller =
        CallerGrants::from_verified(after.context().binding(), [PartyId::new("A").unwrap()]);
    let active = s
        .open()
        .read_page(
            &after,
            &caller,
            &s.lease(),
            crate::PageKind::Active,
            None,
            25,
        )
        .unwrap();
    assert_eq!(active.visible_total, 2);
    assert_eq!(active.first_checkpoint, Some(cursor(10)));
    let mut concurrent = s.create(&cursor(11), "later-old", PAYLOAD);
    let mut archive = baseline.events[0].clone();
    archive.id = "archive-new".into();
    archive.origin = Some(Origin {
        offset: 11,
        snapshot_observation: false,
    });
    archive.witnesses.clear();
    archive.action = Action::ArchiveByPriorContract;
    concurrent.events.push(archive);
    concurrent
        .events
        .extend(create(&after, &s.models[1], &cursor(11), "later-new", PAYLOAD).events);
    assert!(
        s.open()
            .apply_page(&after, &concurrent, &s.lease(), Some(&cursor(10)))
            .unwrap()
    );
    assert!(
        !s.open()
            .apply_page(&after, &concurrent, &s.lease(), Some(&cursor(11)))
            .unwrap()
    );
    assert_eq!(s.count(1, &after), 1);
    // Roll back only the API/model revision using the existing staged rebuild. Canonical new
    // package history and the widened selection remain retained, avoiding destructive rollback.
    s.open()
        .start_model_rebuild(
            &after,
            &s.lease(),
            "rollback",
            &s.namespace,
            &s.models[0].name,
            1,
            Some(2),
        )
        .unwrap();
    for _ in 0..20 {
        let status = s
            .open()
            .model_rebuild(
                &after,
                &s.lease(),
                "rollback",
                crate::RebuildAction::Step,
                1,
            )
            .unwrap();
        if status.phase == "ready" {
            break;
        }
    }
    s.open()
        .model_rebuild(
            &after,
            &s.lease(),
            "rollback",
            crate::RebuildAction::Activate,
            1,
        )
        .unwrap();
    assert_eq!(s.count(0, &after), 2);
    assert_eq!(s.open().checkpoint(&after).unwrap(), Some(cursor(11)));
}
/// Invalid projections, mismatched scopes and concurrent progress leave both selection and rows unchanged.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn failed_upgrade_never_publishes_partial_baselines_or_rewinds_progress() {
    let (s, after) = scenario();
    let manifest = plan(&s, &after);
    let mut oversized = snapshot(&s, &after, "large");
    oversized.events = (0..65)
        .map(|i| snapshot(&s, &after, &format!("large-{i}")).events.remove(0))
        .collect();
    assert!(
        s.open()
            .apply_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest, &oversized)
            .is_err()
    );
    let wrong = lease(&s.plans[0], vec!["B"]);
    assert!(matches!(
        s.open().apply_selection_upgrade(
            &s.plans[0],
            &after,
            &wrong,
            &manifest,
            &snapshot(&s, &after, "denied")
        ),
        Err(StoreError::Authorization)
    ));
    let mut baseline = snapshot(&s, &after, "bad");
    baseline
        .events
        .push(snapshot(&s, &after, "bad-second").events.remove(0));
    baseline.events[1].action = Action::Create(cantongrid_core::value::Value::RawJson("{}".into()));
    assert!(
        s.open()
            .apply_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest, &baseline)
            .is_err()
    );
    assert_eq!(s.count(1, &s.plans[0]), 0);
    assert_eq!(s.open().checkpoint(&s.plans[0]).unwrap(), Some(cursor(10)));
    assert!(
        s.open()
            .apply_selection_upgrade(
                &s.plans[0],
                &after,
                &lease(&s.plans[0], vec!["B"]),
                &manifest,
                &snapshot(&s, &after, "new")
            )
            .is_err()
    );
    s.open()
        .apply_page(
            &s.plans[0],
            &s.create(&cursor(11), "concurrent", PAYLOAD),
            &s.lease(),
            Some(&cursor(10)),
        )
        .unwrap();
    assert!(matches!(
        s.open().apply_selection_upgrade(
            &s.plans[0],
            &after,
            &s.lease(),
            &manifest,
            &snapshot(&s, &after, "new")
        ),
        Err(StoreError::Busy)
    ));
    assert_eq!(s.open().checkpoint(&s.plans[0]).unwrap(), Some(cursor(11)));
}

/// Build one deterministic source page; only the final page acknowledges the complete boundary.
fn staged_page(s: &Scenario, after: &Plan, start: usize, end: usize, last: bool) -> Batch {
    let mut batch = snapshot(s, after, "unused");
    batch.events = (start..end)
        .map(|i| snapshot(s, after, &format!("staged-{i}")).events.remove(0))
        .collect();
    batch.position = if last {
        cursor(10)
    } else {
        serde_json::json!({"snapshot":true,"boundary":10,"through":10,"offset":10,"pk":end})
            .to_string()
    };
    batch
}
/// More than one source page stays private until the whole baseline/model revision is ready.
/// Reopened connections simulate independent CLI processes at every durable transition.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn staged_upgrade_resumes_129_contracts_without_exposing_partial_state() {
    let (s, after) = scenario();
    let manifest = plan(&s, &after);
    let mut state = s
        .open()
        .start_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    for (start, end, last) in [(0, 64, false), (64, 128, false), (128, 129, true)] {
        state = s
            .open()
            .stage_selection_upgrade(
                &s.plans[0],
                &s.lease(),
                &manifest,
                &state.source_cursor,
                &staged_page(&s, &after, start, end, last),
            )
            .unwrap();
    }
    assert_eq!(state.phase, "ready");
    assert_eq!(state.contracts, 129);
    assert_eq!(s.count(1, &after), 0);
    let caller =
        CallerGrants::from_verified(after.context().binding(), [PartyId::new("A").unwrap()]);
    assert_eq!(
        s.open()
            .read_page(
                &s.plans[0],
                &caller,
                &s.lease(),
                crate::PageKind::Active,
                None,
                25
            )
            .unwrap()
            .visible_total,
        1
    );
    s.open()
        .activate_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    for n in 1..=3 {
        state = s
            .open()
            .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
            .unwrap();
        if n < 3 {
            assert!(
                s.open()
                    .read_page(
                        &s.plans[0],
                        &caller,
                        &s.lease(),
                        crate::PageKind::Active,
                        None,
                        25
                    )
                    .is_err()
            );
            assert!(s.open().checkpoint(&s.plans[0]).is_err());
            assert!(matches!(
                s.open().apply_page(
                    &s.plans[0],
                    &s.create(&cursor(11), "stale-worker", PAYLOAD),
                    &s.lease(),
                    Some(&cursor(10))
                ),
                Err(StoreError::Busy)
            ));
        }
    }
    assert_eq!(state.phase, "complete");
    let stats = s.open().stats(&after).unwrap();
    assert_eq!(stats.batches, 1);
    assert_eq!(stats.active_contracts, 130);
    assert_eq!(stats.events, 130);
    assert!(matches!(
        s.open().restore(&after),
        Err(StoreError::UnsupportedRecovery)
    ));
    assert_eq!(s.count(1, &after), 129);
    assert_eq!(s.open().checkpoint(&after).unwrap(), Some(cursor(10)));
    assert_eq!(
        s.open()
            .read_page(
                &after,
                &caller,
                &s.lease(),
                crate::PageKind::Active,
                None,
                25
            )
            .unwrap()
            .visible_total,
        130
    );
    for _ in 0..3 {
        state = s
            .open()
            .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
            .unwrap();
    }
    assert_eq!(state.retained_pages, 0);
    assert_eq!(s.count(1, &after), 129);
}
/// Failure after a previously published private page is recoverable: cancellation removes only
/// this unpublished baseline, retaining the original canonical contracts/checkpoint/model binding.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn staged_upgrade_failure_and_cancellation_restore_the_old_public_prefix() {
    let (s, after) = scenario();
    let manifest = plan(&s, &after);
    let state = s
        .open()
        .start_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    let state = s
        .open()
        .stage_selection_upgrade(
            &s.plans[0],
            &s.lease(),
            &manifest,
            &state.source_cursor,
            &staged_page(&s, &after, 0, 2, false),
        )
        .unwrap();
    let mut bad = staged_page(&s, &after, 2, 3, true);
    bad.events[0].action = Action::Create(cantongrid_core::value::Value::RawJson("{}".into()));
    s.open()
        .stage_selection_upgrade(
            &s.plans[0],
            &s.lease(),
            &manifest,
            &state.source_cursor,
            &bad,
        )
        .unwrap();
    s.open()
        .activate_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    s.open()
        .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    assert!(
        s.open()
            .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
            .is_err()
    );
    assert_eq!(s.count(1, &after), 2);
    let state = s
        .open()
        .selection_upgrade_status(&s.plans[0], &s.lease(), &manifest)
        .unwrap();
    assert_eq!(state.applied, 1);
    s.open()
        .cancel_selection_upgrade(&s.plans[0], &s.lease(), &manifest)
        .unwrap();
    for _ in 0..2 {
        s.open()
            .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
            .unwrap();
    }
    let state = s
        .open()
        .selection_upgrade_status(&s.plans[0], &s.lease(), &manifest)
        .unwrap();
    assert_eq!(state.phase, "cancelled");
    assert_eq!(state.retained_pages, 0);
    assert_eq!(s.count(1, &after), 0);
    assert_eq!(s.open().checkpoint(&s.plans[0]).unwrap(), Some(cursor(10)));
    let caller =
        CallerGrants::from_verified(after.context().binding(), [PartyId::new("A").unwrap()]);
    assert_eq!(
        s.open()
            .read_page(
                &s.plans[0],
                &caller,
                &s.lease(),
                crate::PageKind::Active,
                None,
                25
            )
            .unwrap()
            .visible_total,
        1
    );
}

/// A fully undone job can be explicitly restarted at the same frozen anchor without stale pages.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn cancelled_staging_can_restart_without_reusing_old_source_progress() {
    let (s, after) = scenario();
    let manifest = plan(&s, &after);
    let initial = s
        .open()
        .start_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    let mut stationary = staged_page(&s, &after, 0, 1, false);
    stationary.position = initial.source_cursor.clone();
    assert!(
        s.open()
            .stage_selection_upgrade(
                &s.plans[0],
                &s.lease(),
                &manifest,
                &initial.source_cursor,
                &stationary
            )
            .is_err()
    );
    assert_eq!(
        s.open()
            .selection_upgrade_status(&s.plans[0], &s.lease(), &manifest)
            .unwrap()
            .pages,
        0
    );
    s.open()
        .stage_selection_upgrade(
            &s.plans[0],
            &s.lease(),
            &manifest,
            &initial.source_cursor,
            &staged_page(&s, &after, 0, 1, true),
        )
        .unwrap();
    s.open()
        .cancel_selection_upgrade(&s.plans[0], &s.lease(), &manifest)
        .unwrap();
    s.open()
        .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    let restart = s
        .open()
        .start_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    assert_eq!(restart.phase, "snapshot");
    assert_eq!(restart.pages, 0);
    assert_eq!(restart.source_cursor, initial.source_cursor);
    s.open()
        .cancel_selection_upgrade(&s.plans[0], &s.lease(), &manifest)
        .unwrap();
    let done = s
        .open()
        .step_selection_upgrade(&s.plans[0], &after, &s.lease(), &manifest)
        .unwrap();
    assert_eq!(done.phase, "cancelled");
}
