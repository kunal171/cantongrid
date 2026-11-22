//! Retained LocalNet SQL data, synthetic leases: reconciliation is not current live Ledger acceptance.
use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    config::{Plan, SourceContext, TemplateId},
    lifecycle::Action,
    visibility::{CallerGrants, PartyId},
};
use cantongrid_pqs::{Cursor, lineage_page_env};
use cantongrid_storage::{Store, StoreError, initialize_env};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Instant,
};

/// Clean only this test's isolated generation; never delete retained PQS source rows.
struct Cleanup(String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let Ok(mut client) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL") else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let Ok(Some(row)) = tx.query_opt(
            "SELECT stream_key FROM cantongrid.streams WHERE definition->>'source'=$1",
            &[&self.0],
        ) else {
            return;
        };
        let key: String = row.get(0);
        for table in [
            "lineage_outbox",
            "lineage_edges",
            "creation_events",
            "lineage_pages",
            "exercise_outbox",
            "exercise_events",
            "exercise_pages",
            "outbox",
            "events",
            "contracts",
            "batches",
            "streams",
        ] {
            if tx
                .execute(
                    &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                    &[&key],
                )
                .is_err()
            {
                return;
            }
        }
        let _ = tx.commit();
    }
}

#[test]
#[ignore = "requires retained M0 SQL sources plus owned destination; no Ledger writes"]
fn inspector_records_reconcile_with_retained_source_without_transient_or_hidden_nodes() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let directory = PathBuf::from(std::env::var("CANTONGRID_M0_RUN_DIR").unwrap());
    let read = |name: &str| {
        serde_json::from_slice::<Value>(&std::fs::read(directory.join(name)).unwrap()).unwrap()
    };
    let context = read("context.json");
    let factory = read("transactions.json")["factory"].as_i64().unwrap();
    for scope in ["operator", "a", "b", "ab"] {
        let source = format!("m5-retained-{}-{scope}", std::process::id());
        let _cleanup = Cleanup(source.clone());
        let parties: BTreeSet<_> = context["scopes"][scope]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PartyId::new(p.as_str().unwrap()).unwrap())
            .collect();
        let plan = Plan::pqs(
            SourceContext::new(source, "retained-localnet".into(), 1).unwrap(),
            parties.clone(),
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
        .with_source_binding("a".repeat(64))
        .with_exercises()
        .unwrap()
        .with_lineage()
        .unwrap();
        let variable = format!("CANTONGRID_M1_{}_URL", scope.to_uppercase());
        let end = read(&format!("coverage-{scope}.json"))["query_end"]
            .as_i64()
            .unwrap();
        let mut cursor = serde_json::to_string(&Cursor {
            snapshot: true,
            boundary: factory,
            through: factory,
            offset: factory,
            pk: 0,
        })
        .unwrap();
        let mut store = Store::open_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut witnesses = BTreeMap::new();
        let mut receipts = Vec::new();
        let mut creations = Vec::new();
        let mut archives = BTreeSet::new();
        for _ in 0..32 {
            let now = Instant::now();
            let authority = SourceLease::from_verified(
                plan.context().binding(),
                parties.clone(),
                now,
                MAX_SOURCE_LEASE,
                now + MAX_SOURCE_LEASE,
            )
            .unwrap();
            let Some(page) =
                lineage_page_env(&variable, &plan, &authority, Some(&cursor), 1).unwrap()
            else {
                break;
            };
            let next: Cursor = serde_json::from_str(&page.delivery.batch.position).unwrap();
            if !next.snapshot && next.boundary > end {
                break;
            }
            for event in &page.delivery.batch.events {
                match event.action {
                    Action::Create(_) => {
                        witnesses.insert(event.contract.clone(), event.witnesses.clone());
                    }
                    Action::ArchiveByPriorContract => {
                        archives.insert((
                            event.contract.clone(),
                            event.origin.as_ref().unwrap().offset,
                        ));
                    }
                    _ => {}
                }
            }
            let checkpoint = store.checkpoint(&plan).unwrap();
            store
                .apply_lineage_page(&plan, &page, &authority, checkpoint.as_deref())
                .unwrap();
            cursor = page.delivery.batch.position;
            receipts.extend(page.delivery.exercises);
            creations.extend(page.creations);
        }
        let mut views: Vec<_> = parties
            .iter()
            .cloned()
            .map(|p| BTreeSet::from([p]))
            .collect();
        if parties.len() > 1 {
            views.push(parties.clone());
        }
        for allowed in views {
            let visible = |contract: &str, source_witnesses: &BTreeSet<String>| {
                witnesses
                    .get(contract)
                    .is_some_and(|known: &BTreeSet<PartyId>| !known.is_disjoint(&allowed))
                    && allowed
                        .iter()
                        .any(|p| source_witnesses.contains(p.as_str()))
            };
            let mut expected: BTreeMap<String, Vec<(i32, u8, Value)>> = BTreeMap::new();
            for node in &creations {
                if visible(&node.contract, &node.witnesses) {
                    expected.entry(node.transaction_id.clone()).or_default().push((node.node_id,0,json!({"kind":"create","contractId":node.contract,"template":node.template})));
                }
            }
            for node in &receipts {
                if visible(&node.contract, &node.witnesses) {
                    let entries = expected.entry(node.transaction_id.clone()).or_default();
                    entries.push((node.node_id,1,json!({"kind":"exercise","contractId":node.contract,"template":node.template,"choice":node.choice,"consuming":node.consuming})));
                    if node.consuming && archives.contains(&(node.contract.clone(), node.offset)) {
                        entries.push((node.node_id,2,json!({"kind":"archive","contractId":node.contract,"template":node.template})));
                    }
                }
            }
            let caller = CallerGrants::from_verified(plan.context().binding(), allowed.clone());
            let checkpoint = store.checkpoint(&plan).unwrap();
            for (transaction, mut records) in expected {
                records.sort_by_key(|r| (r.0, r.1));
                let mut expected: Vec<_> = records.into_iter().map(|r| r.2).collect();
                for (index, value) in expected.iter_mut().enumerate() {
                    value["ordinal"] = (index + 1).to_string().into();
                }
                let mut after = None;
                let mut actual = Vec::new();
                loop {
                    let now = Instant::now();
                    let authority = SourceLease::from_verified(
                        plan.context().binding(),
                        parties.clone(),
                        now,
                        MAX_SOURCE_LEASE,
                        now + MAX_SOURCE_LEASE,
                    )
                    .unwrap();
                    let response = store
                        .inspect_transaction(
                            &plan,
                            &caller,
                            &authority,
                            &transaction,
                            after.as_deref(),
                            1,
                        )
                        .unwrap();
                    assert_eq!(response.visible_total as usize, expected.len());
                    actual.extend(response.items);
                    after = response.next_after;
                    if after.is_none() {
                        break;
                    }
                }
                // Report counts on failure, never raw retained contract/party/transaction identities.
                assert!(
                    actual == expected,
                    "retained source record reconciliation failed"
                );
            }
            let now = Instant::now();
            let authority = SourceLease::from_verified(
                plan.context().binding(),
                parties.clone(),
                now,
                MAX_SOURCE_LEASE,
                now + MAX_SOURCE_LEASE,
            )
            .unwrap();
            assert_eq!(
                store
                    .inspect_transaction(&plan, &caller, &authority, "missing", None, 1)
                    .err(),
                Some(StoreError::InspectorUnavailable)
            );
            assert_eq!(store.checkpoint(&plan).unwrap(), checkpoint);
        }
    }
}
