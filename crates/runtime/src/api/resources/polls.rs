//! Long polls have their own budget so waiting consumers cannot occupy all ordinary request slots.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
pub(super) const MAX_POLLS: usize = 4;
pub(super) const PER_PRINCIPAL: usize = 2;

#[derive(Clone)]
pub(super) struct Polls {
    slots: Arc<Semaphore>,
    counts: Arc<Mutex<BTreeMap<String, usize>>>,
}
impl Default for Polls {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(MAX_POLLS)),
            counts: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}
/// Dropping a cancelled HTTP future releases both its global and caller-specific wait budget.
pub(in crate::api) struct Permit {
    _slot: OwnedSemaphorePermit,
    principal: String,
    counts: Arc<Mutex<BTreeMap<String, usize>>>,
}
impl Polls {
    /// Call only after authenticating the principal binding; arbitrary request strings cannot create entries.
    pub(super) fn acquire(&self, principal: &str) -> Result<Permit, &'static str> {
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "poll budget exhausted")?;
        let mut counts = self.counts.lock().map_err(|_| "poll budget unavailable")?;
        let count = counts.entry(principal.into()).or_default();
        if *count >= PER_PRINCIPAL {
            return Err("principal poll budget exhausted");
        }
        *count += 1;
        Ok(Permit {
            _slot: slot,
            principal: principal.into(),
            counts: self.counts.clone(),
        })
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        if let Ok(mut counts) = self.counts.lock()
            && let Some(count) = counts.get_mut(&self.principal)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.remove(&self.principal);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn principal_and_global_limits_release_after_cancellation_and_do_not_retain_identity_churn() {
        let polls = Polls::default();
        let a = polls.acquire("a").unwrap();
        let a2 = polls.acquire("a").unwrap();
        assert!(polls.acquire("a").is_err());
        let b = polls.acquire("b").unwrap();
        let b2 = polls.acquire("b").unwrap();
        assert!(polls.acquire("c").is_err());
        drop(a);
        let c = polls.acquire("c").unwrap();
        drop((a2, b, b2, c));
        assert!(polls.counts.lock().unwrap().is_empty());
        for n in 0..1000 {
            let permit = polls.acquire(&format!("principal{n}")).unwrap();
            let cancelled = async move {
                let _permit = permit;
                std::future::pending::<()>().await
            };
            drop(cancelled);
        }
        assert!(polls.counts.lock().unwrap().is_empty());
        assert_eq!(polls.slots.available_permits(), MAX_POLLS);
    }
}
