//! Root-scoped typed inner joins over current atomic model projections.
mod cursor;
mod sql;
use super::{ModelPage, RelationshipQuery, relationships::active};
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;
const MAX_BYTES: usize = 2 * 1024 * 1024;

impl Store {
    /// Intersect both endpoint policies before counting, fanout, hashing or value encoding.
    /// Only one requested page's selected values crosses into Rust; all reads share one snapshot.
    pub fn read_join(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        query: RelationshipQuery<'_>,
    ) -> Result<ModelPage, StoreError> {
        let RelationshipQuery {
            source,
            target,
            name,
            root,
            after,
            limit,
        } = query;
        if !(1..=25).contains(&limit)
            || root.is_empty()
            || root.len() > 1024
            || root.trim() != root
            || root.chars().any(char::is_control)
        {
            return Err(StoreError::ModelQuery);
        }
        if source.namespace != target.namespace || !super::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        let scope = plan.context().binding();
        let parties: Vec<_> = lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ModelBinding)?;
        let left = active(&mut tx, plan, source)?;
        let right = active(&mut tx, plan, target)?;
        let (source_fields, target_fields) = left
            .model
            .join_fields(name, &right.model)
            .map_err(|_| StoreError::ModelQuery)?;
        let link = left
            .model
            .relationship(name)
            .map_err(|_| StoreError::ModelQuery)?;
        let stream = stream_key(plan).0;
        let left_template =
            serde_json::to_value(&left.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        let right_template =
            serde_json::to_value(&right.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        let root_sql = sql::root(&left.relation());
        let source_values = sql::values("s", &source_fields);
        let row=tx.query_opt(&format!("SELECT CASE WHEN octet_length(value::text)<=2097152 THEN value ELSE NULL END FROM (SELECT {source_values} AS value {root_sql}) permitted"),&[&stream,&parties,&root,&left_template])?.ok_or(StoreError::RelationshipUnavailable)?;
        let source_value: serde_json::Value = row
            .get::<_, Option<serde_json::Value>>(0)
            .ok_or(StoreError::Limit)?;
        let source_bytes = serde_json::to_vec(&source_value)
            .map_err(|_| StoreError::ModelCorrupt)?
            .len();
        let from = sql::targets(
            &right.relation(),
            &link.target.field,
            &link.source_field,
            &root_sql,
        );
        let target_values = sql::values("t", &target_fields);
        let fetch = i64::from(link.max_targets) + 1;
        // The permitted CTE bounds fanout BEFORE projection/hash work. Hash only caller-visible
        // selected columns: a hidden contract change must not invalidate this caller's cursor.
        let metadata=tx.query(&format!("WITH permitted AS MATERIALIZED (SELECT t.*,c.contract_id AS _cg_join_id {from} ORDER BY c.contract_id COLLATE \"C\" LIMIT $6) SELECT t._cg_join_id,encode(sha256(convert_to(({target_values})::text,'UTF8')),'hex'),octet_length(({target_values})::text)::bigint FROM permitted t ORDER BY t._cg_join_id COLLATE \"C\""),&[&stream,&parties,&root,&left_template,&right_template,&fetch])?;
        if metadata.len() > link.max_targets as usize {
            return Err(StoreError::Limit);
        }
        let mut bytes = source_bytes;
        let mut identities = Vec::new();
        for row in &metadata {
            bytes = bytes
                .checked_add(usize::try_from(row.get::<_, i64>(2)).map_err(|_| StoreError::Limit)?)
                .ok_or(StoreError::Limit)?;
            if bytes > MAX_BYTES {
                return Err(StoreError::Limit);
            }
            identities.push((row.get::<_, String>(0), row.get::<_, String>(1)));
        }
        let state = cursor::state(&source_value, &identities)?;
        let after = cursor::after(after, &state)?;
        let visible_total = identities.len().to_string();
        let ids: Vec<_> = identities
            .iter()
            .filter(|(id, _)| after.as_ref().is_none_or(|a| id > a))
            .map(|(id, _)| id.clone())
            .collect();
        let more = ids.len() > limit;
        let page_ids: Vec<_> = ids.into_iter().take(limit).collect();
        let next_after = if more {
            page_ids
                .last()
                .map(|id| cursor::encode(&state, id))
                .transpose()?
        } else {
            None
        };
        let rows=tx.query(&format!("SELECT c.contract_id,{target_values} {from} AND c.contract_id=ANY($6::text[]) ORDER BY c.contract_id COLLATE \"C\""),&[&stream,&parties,&root,&left_template,&right_template,&page_ids])?;
        if rows.len() != page_ids.len() {
            return Err(StoreError::ModelCorrupt);
        }
        let mut items = Vec::new();
        // Repeated source values count for every returned row: 25 × 100 KiB already
        // exceeds 2 MiB even with tiny targets. The state budget above counts the source once.
        let mut response_bytes = 0_usize;
        for row in rows {
            let value = serde_json::json!({"sourceContractId":root,"targetContractId":row.get::<_,String>(0),"relationship":"declared","declaration":name,"declarationRevision":link.revision.to_string(),"sourceModelRevision":source.model.revision.to_string(),"targetModelRevision":target.model.revision.to_string(),"sourceFingerprint":source.fingerprint,"targetFingerprint":target.fingerprint,"coverage":"current_active_projection","depth":1,"joinKind":"inner","sourceValues":source_value,"targetValues":row.get::<_,serde_json::Value>(1)});
            response_bytes = response_bytes
                .checked_add(
                    serde_json::to_vec(&value)
                        .map_err(|_| StoreError::ModelCorrupt)?
                        .len(),
                )
                .ok_or(StoreError::Limit)?;
            if response_bytes > MAX_BYTES {
                return Err(StoreError::Limit);
            }
            items.push(value);
        }
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(ModelPage {
            items,
            visible_total,
            next_after,
            fingerprint: source.fingerprint.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
