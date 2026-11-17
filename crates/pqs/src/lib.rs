//! Read-only inspection of the verified schema-041 profile. This is not live contract ingestion.
#![forbid(unsafe_code)]
use cantongrid_postgres::{DatabaseError, connect_env, postgres::IsolationLevel};
use std::fmt;

pub mod coverage;
mod handoff;
pub use handoff::{Reconciliation, RetainedWindow, reconcile_env};

/// Operator-only SQL snapshot evidence. No payload, party identifier or credential is returned.
#[derive(Debug, serde::Serialize)]
pub struct Inspection {
    pub sql_schema_version: i32,
    pub coverage: coverage::Coverage,
    pub oldest_offset: Option<i64>,
    pub latest_offset: Option<i64>,
    pub pruned_offset: Option<i64>,
    pub active_rows: i64,
    pub create_rows: i64,
    pub archive_rows: i64,
    pub archives_with_witnesses: i64,
    pub history_completeness_verified: bool,
    pub private_lifecycle_ingestion_enabled: bool,
    pub blocker: &'static str,
}

/// Inspect using a dedicated nonowner SQL reader, in one read-only repeatable-read transaction.
/// SQL availability is not source health, a current grant check, or permission to serve cached rows.
pub fn inspect_env(variable: &str) -> Result<Inspection, InspectError> {
    let mut client = connect_env(variable)?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    validate_source(&mut tx)?;
    let row = tx.query_one(
        "SELECT public.oldest_offset(),public.latest_offset(),public.pruned_offset()",
        &[],
    )?;
    let oldest: Option<i64> = row.get(0);
    let latest: Option<i64> = row.get(1);
    let pruned: Option<i64> = row.get(2);
    let (active_rows, create_rows, archive_rows, archives_with_witnesses) = match (oldest, latest) {
        (Some(oldest), Some(latest)) if 0 <= oldest && oldest <= latest => {
            // Explicit arguments avoid schema-041 NULL-return/session-temporal-state behavior.
            let active: i64 = tx
                .query_one("SELECT count(*) FROM public.active(NULL,$1)", &[&latest])?
                .get(0);
            let creates: i64 = tx
                .query_one(
                    "SELECT count(*) FROM public.creates(NULL,$1,$2)",
                    &[&oldest, &latest],
                )?
                .get(0);
            let archives = tx.query_one("SELECT count(*),count(*) FILTER (WHERE cardinality(witnesses)>0) FROM public.archives(NULL,$1,$2)", &[&oldest,&latest])?;
            (active, creates, archives.get(0), archives.get(1))
        }
        (None, None) => (0, 0, 0, 0),
        _ => return Err(InspectError::Bounds),
    };
    tx.commit()?;
    Ok(Inspection {
        sql_schema_version: 41,
        coverage: coverage::Coverage::from_sql_observation(latest, pruned),
        oldest_offset: oldest,
        latest_offset: latest,
        pruned_offset: pruned,
        active_rows,
        create_rows,
        archive_rows,
        archives_with_witnesses,
        history_completeness_verified: false,
        private_lifecycle_ingestion_enabled: false,
        blocker: "Schema 041 archives omit witness evidence; current grants, history coverage and handoff remain unverified. SQL availability does not prove source readiness.",
    })
}

/// Apply the same privilege/schema gate inside every read transaction.
fn validate_source(
    tx: &mut impl cantongrid_postgres::postgres::GenericClient,
) -> Result<(), InspectError> {
    let restricted: bool = tx.query_one(r#"
        SELECT NOT r.rolsuper AND NOT r.rolcreatedb AND NOT r.rolcreaterole
            AND NOT r.rolreplication AND NOT r.rolbypassrls AND d.datdba <> r.oid
            AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member=r.oid)
            AND NOT has_database_privilege(current_user,current_database(),'CREATE,TEMP')
            AND NOT has_schema_privilege(current_user,'public','CREATE')
            AND NOT EXISTS (SELECT 1 FROM pg_proc WHERE pronamespace='public'::regnamespace AND prosecdef)
            AND NOT EXISTS (SELECT 1 FROM pg_class c WHERE c.relnamespace='public'::regnamespace
                AND c.relkind IN ('r','p') AND has_table_privilege(current_user,c.oid,
                'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER'))
        FROM pg_roles r JOIN pg_database d ON d.datname=current_database() WHERE r.rolname=current_user
    "#, &[])?.get(0);
    if !restricted {
        return Err(InspectError::Reader);
    }
    let version: Option<String> = tx.query_opt("SELECT version FROM public.flyway_schema_history WHERE success AND version ~ '^[0-9]+$' ORDER BY installed_rank DESC LIMIT 1", &[])?.map(|r|r.get(0));
    if version.as_deref() != Some("041") {
        return Err(InspectError::Schema);
    }
    Ok(())
}

/// Driver errors stay redacted; a failed probe never becomes permission to use a broader source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectError {
    Database(DatabaseError),
    Reader,
    Schema,
    Bounds,
    Gap,
}
impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => error.fmt(f),
            Self::Reader => f.write_str("PQS inspection requires a restricted nonowner SELECT-only reader with no membership, schema/temp/write or security-definer access"),
            Self::Schema => f.write_str("unsupported PQS SQL schema; only the inspected schema-041 profile is supported"),
            Self::Gap => f.write_str("requested source interval is unavailable or pruned; explicit re-snapshot/coverage decision required"),
            Self::Bounds => f.write_str("PQS temporal bounds are inconsistent; no data was copied"),
        }
    }
}
impl std::error::Error for InspectError {}
impl From<DatabaseError> for InspectError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}
impl From<cantongrid_postgres::postgres::Error> for InspectError {
    fn from(_: cantongrid_postgres::postgres::Error) -> Self {
        Self::Database(DatabaseError::Query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires existing schema-041 LocalNet source and CANTONGRID_PQS_URL"]
    fn existing_read_only_source_reports_coverage_without_enabling_private_ingestion() {
        let report = inspect_env("CANTONGRID_PQS_URL").unwrap();
        assert_eq!(report.sql_schema_version, 41);
        assert!(report.latest_offset >= report.oldest_offset);
        assert!(!report.private_lifecycle_ingestion_enabled);
        assert!(!report.history_completeness_verified);
        assert_eq!(report.archives_with_witnesses, 0);
    }
    #[test]
    #[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
    fn destination_owner_cannot_be_used_as_a_source_reader() {
        assert_eq!(
            inspect_env("CANTONGRID_DESTINATION_URL").unwrap_err(),
            InspectError::Reader
        );
    }

    #[test]
    fn source_settings_do_not_fall_back_to_unrelated_environment() {
        assert_eq!(
            inspect_env("HOME").unwrap_err(),
            InspectError::Database(DatabaseError::Configuration)
        );
    }
}
