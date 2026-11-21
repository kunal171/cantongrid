//! Source evidence and direct edges join the existing atomic writer transaction.
use crate::StoreError;
use cantongrid_core::{
    config::Plan,
    exercise::Exercise,
    lifecycle::Batch,
    lineage::{Creation, DirectEdge, prove_evidence},
};
use cantongrid_postgres::postgres::Transaction;
use sha2::{Digest, Sha256};

/// Verify provenance delivery evidence both on checkpoint reads and again under the writer lock.
pub(crate) fn check_head(
    client: &mut impl cantongrid_postgres::postgres::GenericClient,
    plan: &Plan,
    key: &str,
    revision: i64,
) -> Result<(), StoreError> {
    if plan.includes_lineage()
        && revision > 0
        && client
            .query_opt(
                "SELECT 1 FROM cantongrid.lineage_pages WHERE stream_key=$1 AND revision=$2",
                &[&key, &revision],
            )?
            .is_none()
    {
        return Err(StoreError::CorruptJournal);
    }
    Ok(())
}

/// Validate generation/profile and derive proof records from source evidence, never supplied edges.
pub(crate) fn prepare(
    plan: &Plan,
    batch: &Batch,
    exercises: Option<&Vec<Exercise>>,
    creations: Option<&Vec<Creation>>,
) -> Result<Option<(serde_json::Value, Vec<DirectEdge>)>, StoreError> {
    if plan.includes_lineage() != creations.is_some() {
        return Err(StoreError::InvalidBatch);
    }
    let Some(creations) = creations else {
        return Ok(None);
    };
    let edges = prove_evidence(
        plan,
        batch,
        exercises.ok_or(StoreError::InvalidBatch)?,
        creations,
    )
    .map_err(|_| StoreError::InvalidBatch)?;
    let document = serde_json::to_value(creations).map_err(|_| StoreError::InvalidBatch)?;
    Ok(Some((document, edges)))
}

/// Persist only within the existing stream writer transaction, after its canonical batch exists.
pub(crate) fn persist(
    tx: &mut Transaction<'_>,
    stream: &str,
    revision: i64,
    document: &serde_json::Value,
    edges: &[DirectEdge],
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO cantongrid.lineage_pages VALUES($1,$2,$3)",
        &[&stream, &revision, &document],
    )?;
    for row in document.as_array().ok_or(StoreError::InvalidBatch)? {
        let offset = row["offset"].as_i64().ok_or(StoreError::InvalidBatch)?;
        let node = i32::try_from(row["node_id"].as_i64().ok_or(StoreError::InvalidBatch)?)
            .map_err(|_| StoreError::InvalidBatch)?;
        tx.execute(
            "INSERT INTO cantongrid.creation_events VALUES($1,$2,$3,$4,$5)",
            &[&stream, &offset, &node, &revision, &row],
        )?;
    }
    for edge in edges {
        let key = Sha256::digest(
            serde_json::json!([edge.offset, edge.parent_node, edge.child_node]).to_string(),
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
        let envelope = serde_json::to_value(edge).map_err(|_| StoreError::InvalidBatch)?;
        let clauses =
            serde_json::to_value(&edge.visibility_clauses).map_err(|_| StoreError::InvalidBatch)?;
        tx.execute(
            "INSERT INTO cantongrid.lineage_edges VALUES($1,$2,$3,$4,$5,$6,$7)",
            &[
                &stream,
                &key,
                &revision,
                &edge.parent_contract,
                &edge.child_contract,
                &envelope,
                &clauses,
            ],
        )?;
        tx.execute(
            "INSERT INTO cantongrid.lineage_outbox(stream_key,edge_key) VALUES($1,$2)",
            &[&stream, &key],
        )?;
    }
    Ok(())
}
