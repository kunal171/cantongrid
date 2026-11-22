//! Compare native projected columns against reevaluation without returning financial values or payloads.
use crate::{
    BoundModel, ModelPage, Store, StoreError, journal::stream_key,
    model_projection::relationships::active,
};
use cantongrid_core::{
    authorization::SourceLease, config::Plan, value::Value, visibility::CallerGrants,
};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;

impl Store {
    /// Diagnose one visible contract and exact active model binding in a read-only snapshot.
    /// PostgreSQL compares native records, so Numeric/Int64 never round through JSON numbers.
    pub fn inspect_projection(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        contract: &str,
    ) -> Result<ModelPage, StoreError> {
        if !super::identity(contract) {
            return Err(StoreError::ModelQuery);
        }
        if !crate::model_projection::authorized(plan, lease) {
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
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::InspectorUnavailable)?;
        let model = active(&mut tx, plan, binding)?;
        let key = stream_key(plan).0;
        let template =
            serde_json::to_value(&model.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        let row=tx.query_opt("SELECT payload,active FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2 AND witnesses && $3::text[] AND template=$4",&[&key,&contract,&parties,&template])?.ok_or(StoreError::InspectorUnavailable)?;
        let Value::RawJson(payload) =
            serde_json::from_value(row.get(0)).map_err(|_| StoreError::ModelProjection)?
        else {
            return Err(StoreError::ModelProjection);
        };
        let active: bool = row.get(1);
        let finding = model
            .model
            .diagnose(&payload)
            .map_err(|_| StoreError::ModelCorrupt)?;
        let fields = model.model.evaluate(&payload);
        let expected = if active {
            fields
                .as_ref()
                .ok()
                .and_then(|v| v.as_ref())
                .map(|v| model.model.row_json(&key, contract, v))
                .transpose()
                .map_err(|_| StoreError::ModelCorrupt)?
        } else {
            None
        };
        let present:bool=tx.query_one(&format!("SELECT EXISTS(SELECT 1 FROM {} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2)",model.relation()),&[&key,&contract])?.get(0);
        let projection = if finding.status == "invalid" {
            "evaluation_failed"
        } else if let Some(expected) = expected {
            if !present {
                "missing"
            } else {
                let relation = model.relation();
                let same:bool=tx.query_one(&format!("SELECT EXISTS(SELECT 1 FROM {relation} a,jsonb_populate_record(NULL::{relation},$3) e WHERE a.\"_cg_stream_key\"=$1 AND a.\"_cg_contract_id\"=$2 AND a IS NOT DISTINCT FROM e)"),&[&key,&contract,&expected])?.get(0);
                if same { "consistent" } else { "different" }
            }
        } else if present {
            "unexpected"
        } else {
            "consistent"
        };
        let mut item = serde_json::to_value(finding).map_err(|_| StoreError::ModelCorrupt)?;
        for (name, value) in [
            ("contractId", serde_json::json!(contract)),
            ("namespace", serde_json::json!(binding.namespace)),
            ("active", serde_json::json!(active)),
            ("projection", serde_json::json!(projection)),
            (
                "coverage",
                serde_json::json!("current_projection_diagnostic"),
            ),
        ] {
            item[name] = value;
        }
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(ModelPage {
            items: vec![item],
            visible_total: "1".into(),
            next_after: None,
            fingerprint: binding.fingerprint.clone(),
        })
    }
}
