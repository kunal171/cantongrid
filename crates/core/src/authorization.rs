//! Bounded source grants for pure authorization decisions; no network credential verification here.
use crate::visibility::{CallerGrants, PartyId, ScopeBinding, VisibilityPolicy};
use std::collections::BTreeSet;
use std::fmt;
use std::time::{Duration, Instant};

/// Initial core policy bound, not a measured guarantee of a deployed service.
pub const MAX_SOURCE_LEASE: Duration = Duration::from_secs(15);

/// Non-cloneable, non-deserializable evidence produced after a trusted source grant check.
/// A provider must invalidate it on known scope changes and issue a fresh generation as needed.
#[derive(Debug)]
pub struct SourceLease {
    scope: ScopeBinding,
    parties: BTreeSet<PartyId>,
    checked_at: Instant,
    valid_until: Instant,
    valid: bool,
}

impl SourceLease {
    pub fn from_verified(
        scope: ScopeBinding,
        parties: impl IntoIterator<Item = PartyId>,
        checked_at: Instant,
        valid_for: Duration,
        credential_valid_until: Instant,
    ) -> Result<Self, LeaseError> {
        let parties: BTreeSet<_> = parties.into_iter().collect();
        if parties.is_empty() {
            return Err(LeaseError::MissingGrants);
        }
        if valid_for.is_zero() || valid_for > MAX_SOURCE_LEASE {
            return Err(LeaseError::InvalidDuration);
        }
        if credential_valid_until <= checked_at {
            return Err(LeaseError::ExpiredCredential);
        }
        let valid_until = checked_at
            .checked_add(valid_for)
            .ok_or(LeaseError::InvalidDuration)?
            .min(credential_valid_until);
        Ok(Self {
            scope,
            parties,
            checked_at,
            valid_until,
            valid: true,
        })
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Caller authentication, grant retrieval and payload handling belong to separate layers.
    pub fn authorize(
        &self,
        policy: &VisibilityPolicy,
        caller: &CallerGrants,
        now: Instant,
    ) -> bool {
        self.valid
            && now >= self.checked_at
            && now < self.valid_until
            && policy.scope() == &self.scope
            && policy.allows_within(caller, &self.parties)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseError {
    MissingGrants,
    InvalidDuration,
    ExpiredCredential,
}

impl fmt::Display for LeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingGrants => "source authorization requires current grants",
            Self::InvalidDuration => "source lease duration is outside the permitted bound",
            Self::ExpiredCredential => "source credential has expired",
        })
    }
}

impl std::error::Error for LeaseError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> ScopeBinding {
        ScopeBinding::new("synthetic-source", 1).unwrap()
    }

    fn parties(names: &[&str]) -> Vec<PartyId> {
        names
            .iter()
            .map(|name| PartyId::new(*name).unwrap())
            .collect()
    }

    fn record(names: &[&str]) -> VisibilityPolicy {
        VisibilityPolicy::from_witnesses(scope(), parties(names)).unwrap()
    }

    fn caller(names: &[&str]) -> CallerGrants {
        CallerGrants::from_verified(scope(), parties(names))
    }

    fn lease(names: &[&str], now: Instant) -> SourceLease {
        SourceLease::from_verified(
            scope(),
            parties(names),
            now,
            MAX_SOURCE_LEASE,
            now + Duration::from_secs(60),
        )
        .unwrap()
    }

    #[test]
    fn source_and_caller_must_both_allow_each_input() {
        let now = Instant::now();
        let source = lease(&["A"], now);
        assert!(!source.authorize(&record(&["B"]), &caller(&["A", "B"]), now));
        assert!(!source.authorize(&record(&["A", "B"]), &caller(&["B"]), now));
        assert!(source.authorize(&record(&["A", "B"]), &caller(&["A", "B"]), now));
        let joined = VisibilityPolicy::join(&[record(&["A"]), record(&["B"])]).unwrap();
        assert!(!source.authorize(&joined, &caller(&["A", "B"]), now));
    }

    #[test]
    fn lease_expires_at_the_boundary_without_sleeping() {
        let now = Instant::now();
        let source = lease(&["A"], now);
        let record = record(&["A"]);
        let caller = caller(&["A"]);
        assert!(source.authorize(
            &record,
            &caller,
            now + MAX_SOURCE_LEASE - Duration::from_nanos(1)
        ));
        assert!(!source.authorize(&record, &caller, now + MAX_SOURCE_LEASE));
        assert!(!source.authorize(&record, &caller, now + Duration::from_secs(60)));
        assert!(!source.authorize(&record, &caller, now - Duration::from_secs(1)));
    }

    #[test]
    fn credential_expiry_shortens_the_lease() {
        let now = Instant::now();
        let source = SourceLease::from_verified(
            scope(),
            parties(&["A"]),
            now,
            MAX_SOURCE_LEASE,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert!(!source.authorize(
            &record(&["A"]),
            &caller(&["A"]),
            now + Duration::from_secs(2)
        ));
    }

    #[test]
    fn known_revocation_denies_an_unchanged_cached_record() {
        let now = Instant::now();
        let mut source = lease(&["A", "B"], now);
        let cached = record(&["B"]);
        let caller = caller(&["B"]);
        assert!(source.authorize(&cached, &caller, now));
        source.invalidate();
        assert!(!source.authorize(&cached, &caller, now));
    }

    #[test]
    fn source_context_cannot_be_reused_for_another_generation() {
        let now = Instant::now();
        let source = lease(&["A"], now);
        let newer = ScopeBinding::new("synthetic-source", 2).unwrap();
        let policy = VisibilityPolicy::from_witnesses(newer.clone(), parties(&["A"])).unwrap();
        let caller = CallerGrants::from_verified(newer, parties(&["A"]));
        assert!(!source.authorize(&policy, &caller, now));
    }

    #[test]
    fn absent_grants_invalid_lifetime_and_expired_credentials_are_rejected() {
        let now = Instant::now();
        let future = now + Duration::from_secs(60);
        assert_eq!(
            SourceLease::from_verified(scope(), [], now, MAX_SOURCE_LEASE, future).unwrap_err(),
            LeaseError::MissingGrants
        );
        for duration in [Duration::ZERO, MAX_SOURCE_LEASE + Duration::from_nanos(1)] {
            assert_eq!(
                SourceLease::from_verified(scope(), parties(&["A"]), now, duration, future)
                    .unwrap_err(),
                LeaseError::InvalidDuration
            );
        }
        assert_eq!(
            SourceLease::from_verified(scope(), parties(&["A"]), now, MAX_SOURCE_LEASE, now)
                .unwrap_err(),
            LeaseError::ExpiredCredential
        );
    }
}
