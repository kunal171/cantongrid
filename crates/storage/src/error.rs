//! Redacted persistence errors: driver diagnostics may contain bound private values.
use cantongrid_postgres::DatabaseError;
use std::fmt;

/// Stable failure classes for operator handling and retry decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    Database(DatabaseError),
    UnsafeDestination,
    Schema,
    PlanChanged,
    Busy,
    CorruptJournal,
    UnsupportedRecovery,
    Limit,
    InvalidBatch,
    CommitUnknown,
    Authorization,
    LineageUnavailable,
    ModelDefinition,
    ModelConflict,
    ModelCorrupt,
    ModelMissing,
    ModelSchema,
    ModelBinding,
    ModelQuery,
    ModelRebuild,
    ModelProjection,
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => error.fmt(f),
            _ => f.write_str(match self {
                Self::UnsafeDestination => "destination must be a separately owned database under a restricted non-superuser role, with no PQS schema",
                Self::Schema => "destination schema is absent, incompatible or not owned by this role; initialize/review it explicitly",
                Self::PlanChanged => "persisted source context has a different selection or schema definition; use a new generation and rebuild",
                Self::Busy => "another worker or operator holds a required storage lock; retry after it finishes",
                Self::UnsupportedRecovery => "fixture engine restoration is unavailable for live streams; use SQL reads and checkpoints",
                Self::CorruptJournal => "destination journal/checkpoint is inconsistent; no progress was acknowledged",
                Self::Limit => "bounded reference store limit exceeded; no progress was acknowledged",
                Self::InvalidBatch => "batch failed lifecycle, scope or payload validation; no progress was acknowledged",
                Self::Authorization => "current source/caller authorization is unavailable, mismatched or expired",
                Self::LineageUnavailable => "authorized contract unavailable",
                Self::CommitUnknown => "commit acknowledgement failed; reconnect and retry the same operation before advancing",
                Self::ModelDefinition => "invalid model registry namespace, name, revision or definition",
                Self::ModelConflict => "model revision already has another definition; register a new revision",
                Self::ModelCorrupt => "stored model revision is invalid or its fingerprint differs; review registry integrity",
                Self::ModelMissing => "model revision is not registered in this namespace",
                Self::ModelRebuild => "model rebuild is missing, inconsistent, reserved or not ready for this action",
                Self::ModelQuery => "invalid model query or cursor",
                Self::ModelBinding => "model binding requires an indexed physical source, a selected template and the expected previous revision",
                Self::ModelProjection => "model projection failed; canonical data, projections and checkpoint were not advanced",
                Self::ModelSchema => "model schema or installation metadata differs from the compiled plan; review without automatic repair",
                Self::Database(_) => unreachable!(),
            }),
        }
    }
}
impl std::error::Error for StoreError {}
impl From<DatabaseError> for StoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}
impl From<cantongrid_postgres::postgres::Error> for StoreError {
    fn from(_: cantongrid_postgres::postgres::Error) -> Self {
        Self::Database(DatabaseError::Query)
    }
}
