//! Bounded template-only snapshot and forward polling for the verified schema-041 profile.
use crate::{InspectError, validate_source};
use cantongrid_core::{
    authorization::SourceLease,
    config::{Plan, TemplateId},
    lifecycle::{Action, Batch, Event},
    value::Value,
    visibility::PartyId,
};
use cantongrid_postgres::{connect_env, postgres::IsolationLevel};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Internal durable source cursor. Event PK breaks ties at a participant-local offset.
/// It is never accepted as an API cursor and cannot be moved across plan generations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub snapshot: bool,
    pub boundary: i64,
    pub through: i64,
    pub offset: i64,
    pub pk: i64,
}

/// Read one bounded page in a repeatable-read, read-only transaction. The caller supplies
/// a freshly verified lease and must check it again before committing destination writes.
pub fn page_env(
    variable: &str,
    plan: &Plan,
    lease: &SourceLease,
    checkpoint: Option<&str>,
    limit: usize,
) -> Result<Option<Batch>, InspectError> {
    if !plan.is_live() || !(1..=64).contains(&limit) {
        return Err(InspectError::Bounds);
    }
    let scope = plan.context().binding();
    let parties: Vec<String> = lease
        .source_parties(&scope, Instant::now())
        .ok_or(InspectError::Authorization)?
        .iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    // A valid broader lease must not widen this plan's configured SQL selection.
    let expected: Vec<_> = plan
        .source_parties()
        .map(|p| p.as_str().to_owned())
        .collect();
    if parties != expected {
        return Err(InspectError::Authorization);
    }
    let mut client = connect_env(variable)?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    validate_source(&mut tx)?;
    let bounds = tx.query_one(
        "SELECT public.oldest_offset(),public.latest_offset(),public.pruned_offset()",
        &[],
    )?;
    let oldest: i64 = bounds
        .get::<_, Option<i64>>(0)
        .ok_or(InspectError::Bounds)?;
    let latest: i64 = bounds
        .get::<_, Option<i64>>(1)
        .ok_or(InspectError::Bounds)?;
    let pruned: Option<i64> = bounds.get(2);
    let mut cursor: Cursor = match checkpoint {
        Some(text) => serde_json::from_str(text).map_err(|_| InspectError::Bounds)?,
        None => Cursor {
            snapshot: true,
            boundary: latest,
            through: latest,
            offset: latest,
            pk: 0,
        },
    };
    validate_cursor(&cursor, oldest, latest, pruned)?;
    if !cursor.snapshot && cursor.through == cursor.boundary {
        if latest == cursor.boundary {
            return Ok(None);
        }
        cursor.through = latest;
    }
    let packages: Vec<_> = plan.templates().map(|t| t.package_id.clone()).collect();
    let modules: Vec<_> = plan.templates().map(|t| t.module.clone()).collect();
    let entities: Vec<_> = plan.templates().map(|t| t.entity.clone()).collect();
    // Selection is an exact zipped triple, not independent package/module/entity IN lists.
    let rows = tx.query(
        include_str!("reader.sql"),
        &[
            &cursor.snapshot,
            &cursor.boundary,
            &cursor.through,
            &cursor.offset,
            &cursor.pk,
            &parties,
            &packages,
            &modules,
            &entities,
            &(limit as i64 + 1),
        ],
    )?;
    let more = rows.len() > limit;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| InspectError::Bounds)?
        .as_micros();
    let observed = i64::try_from(now).map_err(|_| InspectError::Bounds)?;
    let mut events = Vec::new();
    for row in rows.into_iter().take(limit) {
        let offset: i64 = row.get(0);
        let pk: i64 = row.get(1);
        let archive: bool = row.get(2);
        let fqn: String = row.get(5);
        let components: Vec<_> = fqn.split(':').collect();
        if components.len() != 3 {
            return Err(InspectError::Bounds);
        }
        let template = TemplateId {
            package_id: row.get(6),
            module: components[1].into(),
            entity: components[2].into(),
        };
        template.validate().map_err(|_| InspectError::Bounds)?;
        let witnesses: BTreeSet<_> = row
            .get::<_, Vec<String>>(7)
            .into_iter()
            .map(PartyId::new)
            .collect::<Result<_, _>>()
            .map_err(|_| InspectError::Bounds)?;
        let action = if archive {
            if !witnesses.is_empty() {
                return Err(InspectError::Bounds);
            }
            Action::ArchiveByPriorContract
        } else {
            Value::RawJson(row.get::<_, Option<String>>(8).ok_or(InspectError::Limit)?)
                .validate()
                .map_err(|_| InspectError::Bounds)?;
            Action::Create(Value::RawJson(
                row.get::<_, Option<String>>(8).ok_or(InspectError::Limit)?,
            ))
        };
        events.push(Event {
            id: row.get(3),
            contract: row.get(4),
            template,
            witnesses,
            observed_at_micros: observed,
            ledger_at_micros: row.get(9),
            origin: Some(cantongrid_core::lifecycle::Origin {
                offset: row.get(10),
                snapshot_observation: cursor.snapshot,
            }),
            action,
        });
        cursor.offset = offset;
        cursor.pk = pk;
    }
    if !more {
        cursor.snapshot = false;
        cursor.boundary = cursor.through;
        cursor.offset = cursor.through;
        cursor.pk = i64::MAX;
    }
    let position = serde_json::to_string(&cursor).map_err(|_| InspectError::Bounds)?;
    tx.commit()?;
    if lease.source_parties(&scope, Instant::now()).is_none() {
        return Err(InspectError::Authorization);
    }
    Ok(Some(Batch {
        context: plan.context().clone(),
        position,
        events,
    }))
}

/// Reject regressed, unavailable or pruned cursors; never skip forward to repair a gap.
fn validate_cursor(
    c: &Cursor,
    oldest: i64,
    latest: i64,
    pruned: Option<i64>,
) -> Result<(), InspectError> {
    if oldest < 0
        || latest < oldest
        || c.boundary < 0
        || c.through < c.boundary
        || c.through > latest
        || c.offset < c.boundary
        || c.offset > c.through
        || c.pk < 0
        || (c.snapshot && c.boundary != c.through)
    {
        return Err(InspectError::Bounds);
    }
    if c.boundary < oldest || pruned.is_some_and(|p| c.boundary <= p) {
        return Err(InspectError::Gap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Reject wider/narrower leases before even opening SQL, not just at destination commit.
    #[test]
    fn source_lease_must_match_the_exact_selected_party_set() {
        use cantongrid_core::{authorization::MAX_SOURCE_LEASE, config::SourceContext};
        use std::time::Duration;
        let party = |name: &str| PartyId::new(name).unwrap();
        let plan = Plan::pqs(
            SourceContext::new("scope-check".into(), "synthetic".into(), 1).unwrap(),
            [party("A"), party("B")].into(),
            [TemplateId {
                package_id: "0".repeat(64),
                module: "Example".into(),
                entity: "Record".into(),
            }]
            .into(),
        )
        .unwrap()
        .with_source_binding("a".repeat(64));
        for names in [vec!["A"], vec!["A", "B", "C"], vec!["C"]] {
            let lease = SourceLease::from_verified(
                plan.context().binding(),
                names.into_iter().map(party),
                Instant::now(),
                MAX_SOURCE_LEASE,
                Instant::now() + Duration::from_secs(10),
            )
            .unwrap();
            assert!(matches!(
                page_env("CANTONGRID_UNUSED_AUDIT_SOURCE", &plan, &lease, None, 1),
                Err(InspectError::Authorization)
            ));
        }
    }

    #[test]
    fn cursor_keeps_same_offset_ties_and_fails_on_pruning_or_regression() {
        let c = Cursor {
            snapshot: false,
            boundary: 10,
            through: 20,
            offset: 15,
            pk: 42,
        };
        assert!(validate_cursor(&c, 0, 20, None).is_ok());
        assert!(validate_cursor(&c, 11, 20, None).is_err());
        assert!(validate_cursor(&c, 0, 19, None).is_err());
        assert!(validate_cursor(&c, 0, 20, Some(10)).is_err());
        let decoded: Cursor = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!((decoded.offset, decoded.pk), (15, 42));
    }
}
