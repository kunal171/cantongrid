//! Validate observed source intervals without turning a missing journal into an empty history.
use crate::{StoreError, upgrade_boundary};
use cantongrid_postgres::postgres::Transaction;
use serde::Deserialize;

/// The supported schema041 cursor profile. Other position formats have no temporal proof here.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    snapshot: bool,
    boundary: i64,
    through: i64,
    offset: i64,
    pk: i64,
}
impl Position {
    /// Source offsets are ordered locally; PKs break same-offset ties and are never public cursors.
    fn decode(text: &str) -> Result<Self, StoreError> {
        let p: Self = serde_json::from_str(text).map_err(|_| StoreError::TemporalUnavailable)?;
        if p.boundary < 0
            || p.through < p.boundary
            || p.offset < p.boundary
            || p.offset > p.through
            || p.pk < 0
            || (p.snapshot && p.boundary != p.through)
        {
            return Err(StoreError::TemporalUnavailable);
        }
        Ok(p)
    }
}
/// Exact coverage for this configured selection; additive baselines raise the common floor.
pub(super) struct Coverage {
    pub floor_position: String,
    pub through: i64,
}
/// Check every bounded journal position and indexed event origin in one stable read snapshot.
/// This prototype deliberately fails above its budget instead of silently truncating history.
pub(super) fn observed(
    tx: &mut Transaction<'_>,
    key: &str,
    revision: i64,
    checkpoint: &str,
    requested: i64,
) -> Result<Coverage, StoreError> {
    let rows = tx.query(
        "SELECT revision,position,NOT EXISTS(SELECT 1 FROM jsonb_array_elements(b.envelope->'events') e WHERE NOT EXISTS(SELECT 1 FROM cantongrid.events x WHERE x.stream_key=b.stream_key AND x.event_id=e->>'id' AND x.contract_id=e->>'contract' AND x.position=b.position)) FROM cantongrid.batches b WHERE stream_key=$1 ORDER BY revision LIMIT 4097", &[&key])?;
    if rows.is_empty() || rows.len() > 4096 || rows.len() as i64 != revision {
        return Err(StoreError::TemporalUnavailable);
    }
    let mut previous: Option<Position> = None;
    let mut floor_position: String = rows[0].get(1);
    let mut floor = Position::decode(&floor_position)?.boundary;
    for (index, row) in rows.iter().enumerate() {
        if row.get::<_, i64>(0) != index as i64 + 1 || !row.get::<_, bool>(2) {
            return Err(StoreError::TemporalUnavailable);
        }
        let p = Position::decode(&row.get::<_, String>(1))?;
        if let Some(old) = &previous {
            // An interval may advance its upper bound only after the preceding one completed.
            if (!old.snapshot && p.snapshot)
                || p.boundary < old.boundary
                || (old.snapshot && p.through != old.through)
                || (old.through != old.boundary && p.through != old.through)
                || (!old.snapshot
                    && (p.boundary, p.offset, p.pk) < (old.boundary, old.offset, old.pk))
            {
                return Err(StoreError::TemporalUnavailable);
            }
        }
        previous = Some(p);
    }
    let head = previous.ok_or(StoreError::TemporalUnavailable)?;
    if head.snapshot || rows.last().map(|r| r.get::<_, String>(1)).as_deref() != Some(checkpoint) {
        return Err(StoreError::TemporalUnavailable);
    }
    // Selection upgrades retain old records but establish only a new snapshot for added templates.
    let upgrades = tx.query("SELECT manifest->>'checkpoint' FROM cantongrid.selection_upgrades WHERE stream_key=$1 LIMIT 65", &[&key])?;
    if upgrades.len() > 64 {
        return Err(StoreError::TemporalUnavailable);
    }
    for row in upgrades {
        let position: String = row.get(0);
        let boundary = upgrade_boundary(&position).map_err(|_| StoreError::TemporalUnavailable)?;
        if boundary > floor {
            floor = boundary;
            floor_position = position;
        }
    }
    if requested < floor || requested > head.boundary {
        return Err(StoreError::TemporalUnavailable);
    }
    let events=tx.query("SELECT envelope#>'{events,0,origin}',kind FROM cantongrid.events WHERE stream_key=$1 LIMIT 8193", &[&key])?;
    if events.len() > 8192 {
        return Err(StoreError::TemporalUnavailable);
    }
    for row in events {
        let origin: serde_json::Value = row.get(0);
        let origin: cantongrid_core::lifecycle::Origin =
            serde_json::from_value(origin).map_err(|_| StoreError::TemporalUnavailable)?;
        if origin.offset < 0
            || origin.offset > head.through
            || (origin.snapshot_observation && origin.offset > floor)
            || (row.get::<_, String>(1) == "archive" && origin.snapshot_observation)
        {
            return Err(StoreError::TemporalUnavailable);
        }
    }
    let corrupt:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.contracts c LEFT JOIN cantongrid.events x ON x.stream_key=c.stream_key AND x.event_id=c.create_event_id LEFT JOIN cantongrid.events a ON a.stream_key=c.stream_key AND a.event_id=c.archive_event_id WHERE c.stream_key=$1 AND (x.kind IS DISTINCT FROM 'create' OR x.contract_id IS DISTINCT FROM c.contract_id OR c.active<>(c.archive_event_id IS NULL) OR (c.archive_event_id IS NOT NULL AND (a.kind IS DISTINCT FROM 'archive' OR a.contract_id IS DISTINCT FROM c.contract_id))))", &[&key])?.get(0);
    if corrupt {
        return Err(StoreError::TemporalUnavailable);
    }
    Ok(Coverage {
        floor_position,
        through: head.boundary,
    })
}
