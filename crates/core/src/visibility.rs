//! Pure visibility evaluation over trusted metadata, separate from authentication and source leases.
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PartyId(String);

impl PartyId {
    pub fn new(value: impl Into<String>) -> Result<Self, PolicyError> {
        let value = value.into();
        validate_identity(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PartyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PartyId([redacted])")
    }
}

/// Trusted source/authorization context. Generation changes invalidate old policies and grants.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopeBinding {
    id: String,
    generation: u64,
}

impl ScopeBinding {
    pub fn new(id: impl Into<String>, generation: u64) -> Result<Self, PolicyError> {
        let id = id.into();
        validate_identity(&id)?;
        Ok(Self { id, generation })
    }
}

impl fmt::Debug for ScopeBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopeBinding")
            .field("id", &"[redacted]")
            .field("generation", &self.generation)
            .finish()
    }
}

/// Only build this after authenticating the caller and resolving its current permitted parties.
/// This type intentionally has no deserializer and does not authenticate client-supplied values.
#[derive(Clone, Debug)]
pub struct CallerGrants {
    scope: ScopeBinding,
    parties: BTreeSet<PartyId>,
}

impl CallerGrants {
    pub fn from_verified(scope: ScopeBinding, parties: impl IntoIterator<Item = PartyId>) -> Self {
        Self {
            scope,
            parties: parties.into_iter().collect(),
        }
    }
}

/// AND across contributing inputs; OR across permitted parties for each individual input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityPolicy {
    scope: ScopeBinding,
    clauses: BTreeSet<BTreeSet<PartyId>>,
}

impl VisibilityPolicy {
    pub fn from_witnesses(
        scope: ScopeBinding,
        witnesses: impl IntoIterator<Item = PartyId>,
    ) -> Result<Self, PolicyError> {
        let clause: BTreeSet<_> = witnesses.into_iter().collect();
        if clause.is_empty() {
            return Err(PolicyError::MissingEvidence);
        }
        Ok(Self {
            scope,
            clauses: BTreeSet::from([clause]),
        })
    }

    /// Retain the authorization dependency of every contributing input, including derived ones.
    pub fn join(inputs: &[Self]) -> Result<Self, PolicyError> {
        let first = inputs.first().ok_or(PolicyError::MissingEvidence)?;
        if inputs.iter().any(|input| input.scope != first.scope) {
            return Err(PolicyError::ScopeMismatch);
        }
        let clauses = inputs
            .iter()
            .flat_map(|input| input.clauses.iter().cloned())
            .collect();
        Ok(Self {
            scope: first.scope.clone(),
            clauses,
        })
    }

    /// Pure policy check only. A serving caller must also require a current source-authorization lease.
    pub fn allows(&self, caller: &CallerGrants) -> bool {
        self.matches_parties(&caller.scope, |party| caller.parties.contains(party))
    }

    pub(crate) fn scope(&self) -> &ScopeBinding {
        &self.scope
    }

    pub(crate) fn allows_within(&self, caller: &CallerGrants, source: &BTreeSet<PartyId>) -> bool {
        self.matches_parties(&caller.scope, |party| {
            caller.parties.contains(party) && source.contains(party)
        })
    }

    fn matches_parties(&self, scope: &ScopeBinding, can_access: impl Fn(&PartyId) -> bool) -> bool {
        self.scope == *scope
            && !self.clauses.is_empty()
            && self
                .clauses
                .iter()
                .all(|clause| clause.iter().any(&can_access))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    InvalidIdentity,
    MissingEvidence,
    ScopeMismatch,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidIdentity => "identity is empty or contains a control character",
            Self::MissingEvidence => "visibility requires nonempty input evidence",
            Self::ScopeMismatch => "input authorization contexts differ",
        })
    }
}

impl std::error::Error for PolicyError {}

fn validate_identity(value: &str) -> Result<(), PolicyError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(PolicyError::InvalidIdentity)
    } else {
        Ok(())
    }
}

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

    #[test]
    fn joint_record_is_visible_to_either_authorized_party() {
        let joint = record(&["A", "B"]);
        for allowed in [&["A"][..], &["B"][..], &["A", "B"][..]] {
            assert!(joint.allows(&caller(allowed)));
        }
        assert!(!joint.allows(&caller(&[])));
        assert!(!joint.allows(&caller(&["C"])));
    }

    #[test]
    fn independent_records_require_all_input_dependencies() {
        let combined = VisibilityPolicy::join(&[record(&["A"]), record(&["B"])]).unwrap();
        assert!(!combined.allows(&caller(&["A"])));
        assert!(!combined.allows(&caller(&["B"])));
        assert!(combined.allows(&caller(&["A", "B"])));
    }

    #[test]
    fn nested_derivations_preserve_conjunctions_and_deduplicate_identical_clauses() {
        let first = VisibilityPolicy::join(&[record(&["A"]), record(&["B"])]).unwrap();
        let nested = VisibilityPolicy::join(&[first.clone(), record(&["A"])]).unwrap();
        assert_eq!(nested, first);
        assert!(!nested.allows(&caller(&["A"])));
        assert!(nested.allows(&caller(&["A", "B"])));
    }

    #[test]
    fn empty_evidence_cannot_become_public_access() {
        assert_eq!(
            VisibilityPolicy::from_witnesses(scope(), []),
            Err(PolicyError::MissingEvidence)
        );
        assert_eq!(
            VisibilityPolicy::join(&[]),
            Err(PolicyError::MissingEvidence)
        );
    }

    #[test]
    fn source_and_generation_mismatches_deny_reads_and_joins() {
        let original = record(&["A"]);
        for different in [
            ScopeBinding::new("other-source", 1).unwrap(),
            ScopeBinding::new("synthetic-source", 2).unwrap(),
        ] {
            assert!(!original.allows(&CallerGrants::from_verified(
                different.clone(),
                parties(&["A"])
            )));
            let foreign = VisibilityPolicy::from_witnesses(different, parties(&["A"])).unwrap();
            assert_eq!(
                VisibilityPolicy::join(&[original.clone(), foreign]),
                Err(PolicyError::ScopeMismatch)
            );
        }
    }

    #[test]
    fn identity_is_opaque_validated_and_redacted() {
        assert!(PartyId::new("").is_err());
        assert!(PartyId::new("A\nB").is_err());
        assert!(ScopeBinding::new("", 1).is_err());
        let identity = PartyId::new("private-party").unwrap();
        assert_eq!(identity.as_str(), "private-party");
        assert!(!format!("{identity:?}").contains("private-party"));
        assert!(
            !format!("{:?}", ScopeBinding::new("private-source", 1).unwrap())
                .contains("private-source")
        );
    }
}
