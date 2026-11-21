//! Read-only retained M0 source controls. Leases here are synthetic, not live grant acceptance.
use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    config::{Plan, SourceContext, TemplateId},
    visibility::PartyId,
};
use cantongrid_pqs::{Cursor, exercise_page_env};
use std::{path::PathBuf, time::Instant};

#[test]
#[ignore = "requires retained M0 LocalNet source databases and CANTONGRID_M0_RUN_DIR; no Ledger writes"]
fn retained_scoped_sources_preserve_atomic_transactions_and_receipt_replay() {
    let directory = PathBuf::from(std::env::var("CANTONGRID_M0_RUN_DIR").unwrap());
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(directory.join(name)).unwrap()).unwrap()
    };
    let context = read("context.json");
    let transactions = read("transactions.json");
    let factory = transactions["factory"].as_i64().unwrap();
    for scope in ["operator", "a", "b", "ab"] {
        let variable = format!("CANTONGRID_M1_{}_URL", scope.to_uppercase());
        let plan = Plan::pqs(
            SourceContext::new(format!("m1-{scope}"), "retained-localnet".into(), 1).unwrap(),
            context["scopes"][scope]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| PartyId::new(p.as_str().unwrap()).unwrap())
                .collect(),
            ["Factory", "Child", "Relay"]
                .into_iter()
                .map(|entity| TemplateId {
                    package_id: context["package"].as_str().unwrap().into(),
                    module: "ExerciseProbe".into(),
                    entity: entity.into(),
                })
                .collect(),
        )
        .unwrap()
        .with_exercises()
        .unwrap();
        let now = Instant::now();
        let lease = SourceLease::from_verified(
            plan.context().binding(),
            plan.source_parties().cloned(),
            now,
            MAX_SOURCE_LEASE,
            now + MAX_SOURCE_LEASE,
        )
        .unwrap();
        let mut cursor = serde_json::to_string(&Cursor {
            snapshot: true,
            boundary: factory,
            through: factory,
            offset: factory,
            pk: 0,
        })
        .unwrap();
        let expected = read(&format!("pqs-{scope}.json"));
        let mut receipts = vec![];
        let mut baseline = 0;
        for _ in 0..32 {
            let Some(page) = exercise_page_env(&variable, &plan, &lease, Some(&cursor), 1).unwrap()
            else {
                break;
            };
            // Restart at the previous acknowledged cursor: source receipts must be identical.
            let retry = exercise_page_env(&variable, &plan, &lease, Some(&cursor), 1)
                .unwrap()
                .unwrap();
            assert_eq!(page.exercises, retry.exercises);
            let offsets: std::collections::BTreeSet<_> = page
                .exercises
                .iter()
                .map(|e| e.offset)
                .chain(page.batch.events.iter().filter_map(|e| {
                    e.origin
                        .as_ref()
                        .filter(|o| !o.snapshot_observation)
                        .map(|o| o.offset)
                }))
                .collect();
            assert!(offsets.len() <= 1);
            baseline += page
                .batch
                .events
                .iter()
                .filter(|e| e.origin.as_ref().unwrap().snapshot_observation)
                .count();
            receipts.extend(page.exercises);
            cursor = page.batch.position;
        }
        assert_eq!(baseline, 1);
        assert_eq!(
            receipts.len(),
            expected["exercises"].as_array().unwrap().len()
        );
        for receipt in receipts {
            let original = expected["exercises"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| {
                    r["exercise_event_id"]["offset"].as_i64() == Some(receipt.offset)
                        && r["exercise_event_id"]["node_id"].as_i64()
                            == Some(i64::from(receipt.node_id))
                })
                .unwrap();
            assert!(
                serde_json::from_str::<serde_json::Value>(&receipt.argument).unwrap()
                    == original["argument"]
            );
            assert!(
                serde_json::from_str::<serde_json::Value>(&receipt.result).unwrap()
                    == original["result"]
            );
            assert!(
                receipt.last_descendant_node_id
                    == original["last_descendant_node_id"].as_i64().unwrap() as i32
            );
        }
    }
}
