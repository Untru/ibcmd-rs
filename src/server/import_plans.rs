//! One-shot confirmation storage, not SQL profile or publication authority.
//!
//! The caller owns payload validation and supplies monotonic time. Reconnect and
//! shutdown must call `clear`; no request may override the server TTL or capacity.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_PENDING: usize = 4;
const PLAN_TTL: Duration = Duration::from_secs(300);
const TOKEN_ATTEMPTS: usize = 8;

struct Pending<P> {
    target: String,
    epoch: u64,
    created: Instant,
    expires: Instant,
    payload: P,
}

pub(super) struct Registry<P> {
    pending: HashMap<Uuid, Pending<P>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum InsertError {
    ClockOverflow,
    TokenUnavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TakeError {
    Missing,
    ClockReversed,
    Expired,
    TargetChanged,
    EpochChanged,
}

impl<P> Registry<P> {
    pub(super) fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    pub(super) fn insert(
        &mut self,
        target: &str,
        epoch: u64,
        payload: P,
        now: Instant,
    ) -> Result<Uuid, InsertError> {
        // Refusal must not discard an already-confirmable plan.
        let expires = now
            .checked_add(PLAN_TTL)
            .ok_or(InsertError::ClockOverflow)?;
        let token = (0..TOKEN_ATTEMPTS)
            .map(|_| Uuid::new_v4())
            .find(|token| !self.pending.contains_key(token))
            .ok_or(InsertError::TokenUnavailable)?;
        self.pending
            .retain(|_, entry| entry.target != target && now < entry.expires);
        // Preserve the existing fifth-distinct-plan policy: clear all old plans.
        if self.pending.len() >= MAX_PENDING {
            self.pending.clear();
        }
        self.pending.insert(
            token,
            Pending {
                target: target.to_owned(),
                epoch,
                created: now,
                expires,
                payload,
            },
        );
        Ok(token)
    }

    /// Every attempted consumption burns the token before any binding check.
    /// Returned payloads still require their typed source/readset/profile guards.
    pub(super) fn take(
        &mut self,
        token: &Uuid,
        current_target: &str,
        current_epoch: u64,
        now: Instant,
    ) -> Result<P, TakeError> {
        let plan = self.pending.remove(token);
        self.pending.retain(|_, entry| now < entry.expires);
        let plan = plan.ok_or(TakeError::Missing)?;
        if now < plan.created {
            return Err(TakeError::ClockReversed);
        }
        if now >= plan.expires {
            return Err(TakeError::Expired);
        }
        if plan.target != current_target {
            return Err(TakeError::TargetChanged);
        }
        if plan.epoch != current_epoch {
            return Err(TakeError::EpochChanged);
        }
        Ok(plan.payload)
    }

    /// Called by the owner on reconnect, settings replacement and shutdown.
    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DropProbe(std::rc::Rc<std::cell::Cell<usize>>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    fn probe() -> (DropProbe, std::rc::Rc<std::cell::Cell<usize>>) {
        let drops = std::rc::Rc::new(std::cell::Cell::new(0));
        (DropProbe(drops.clone()), drops)
    }

    #[test]
    fn insert_releases_expired_other_targets_before_capacity_policy() {
        let now = Instant::now();
        let later = now.checked_add(Duration::from_secs(1)).unwrap();
        let expiry = now.checked_add(PLAN_TTL).unwrap();
        let mut registry = Registry::new();
        let (expired, expired_drops) = probe();
        registry.insert("expired", 1, expired, now).unwrap();
        let mut live = Vec::new();
        for target in ["a", "b", "c"] {
            let (payload, drops) = probe();
            let token = registry.insert(target, 1, payload, later).unwrap();
            live.push((target, token, drops));
        }
        let (new, new_drops) = probe();
        registry.insert("new", 1, new, expiry).unwrap();
        assert_eq!(expired_drops.get(), 1);
        assert_eq!(registry.pending.len(), MAX_PENDING);
        for (target, token, drops) in live {
            assert_eq!(drops.get(), 0, "live target was evicted: {target}");
            drop(registry.take(&token, target, 1, expiry).unwrap());
            assert_eq!(drops.get(), 1);
        }
        assert_eq!(new_drops.get(), 0);
        registry.clear();
        assert_eq!(new_drops.get(), 1);
    }

    #[test]
    fn missing_take_releases_expired_payloads_but_preserves_future_plans() {
        let now = Instant::now();
        let expiry = now.checked_add(PLAN_TTL).unwrap();
        let future = expiry.checked_add(Duration::from_secs(1)).unwrap();
        let mut registry = Registry::new();
        // Insert the future-dated entry first: inserting it last would itself
        // reap the expired entry, hiding whether a missing take reaps anything.
        let (payload, future_drops) = probe();
        let token = registry.insert("future", 1, payload, future).unwrap();
        let (expired, expired_drops) = probe();
        registry.insert("expired", 1, expired, now).unwrap();
        assert!(matches!(
            registry.take(&Uuid::nil(), "missing", 1, expiry),
            Err(TakeError::Missing)
        ));
        assert_eq!(expired_drops.get(), 1);
        assert_eq!(future_drops.get(), 0);
        assert!(matches!(
            registry.take(&token, "future", 1, expiry),
            Err(TakeError::ClockReversed)
        ));
        assert_eq!(future_drops.get(), 1);
    }

    #[test]
    fn requested_expired_token_keeps_its_error_while_other_payloads_are_released() {
        let now = Instant::now();
        let expiry = now.checked_add(PLAN_TTL).unwrap();
        let mut registry = Registry::new();
        let (requested, requested_drops) = probe();
        let token = registry.insert("requested", 1, requested, now).unwrap();
        let (other, other_drops) = probe();
        registry.insert("other", 1, other, now).unwrap();
        assert!(matches!(
            registry.take(&token, "requested", 1, expiry),
            Err(TakeError::Expired)
        ));
        assert_eq!(requested_drops.get(), 1);
        assert_eq!(other_drops.get(), 1);
        assert!(registry.pending.is_empty());
    }

    #[test]
    fn exact_expiry_boundary_is_refused_and_consumed() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let token = registry.insert("db", 1, 42, now).unwrap();
        let expires = now.checked_add(PLAN_TTL).unwrap();
        assert_eq!(
            registry.take(&token, "db", 1, expires),
            Err(TakeError::Expired)
        );
        assert_eq!(registry.take(&token, "db", 1, now), Err(TakeError::Missing));
    }

    #[test]
    fn just_before_expiry_returns_owned_payload_once() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let token = registry
            .insert("db", 7, String::from("opaque plan"), now)
            .unwrap();
        let before = now.checked_add(PLAN_TTL - Duration::from_nanos(1)).unwrap();
        assert_eq!(
            registry.take(&token, "db", 7, before).unwrap(),
            "opaque plan"
        );
        assert_eq!(registry.take(&token, "db", 7, now), Err(TakeError::Missing));
    }

    #[test]
    fn target_and_epoch_refusals_burn_tokens() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let target = registry.insert("db", 1, 10, now).unwrap();
        assert_eq!(
            registry.take(&target, "DB", 1, now),
            Err(TakeError::TargetChanged)
        );
        assert_eq!(
            registry.take(&target, "db", 1, now),
            Err(TakeError::Missing)
        );
        let epoch = registry.insert("db", 1, 20, now).unwrap();
        assert_eq!(
            registry.take(&epoch, "db", 2, now),
            Err(TakeError::EpochChanged)
        );
        assert_eq!(registry.take(&epoch, "db", 1, now), Err(TakeError::Missing));
    }

    #[test]
    fn clock_reversal_is_refused_and_consumed() {
        let now = Instant::now();
        let later = now.checked_add(Duration::from_secs(1)).unwrap();
        let mut registry = Registry::new();
        let token = registry.insert("db", 1, 10, later).unwrap();
        assert_eq!(
            registry.take(&token, "db", 1, now),
            Err(TakeError::ClockReversed)
        );
        assert_eq!(
            registry.take(&token, "db", 1, later),
            Err(TakeError::Missing)
        );
    }

    #[test]
    fn overflow_does_not_replace_or_clear_existing_plans() {
        let now = Instant::now();
        // Find the largest representable whole-second offset on this platform.
        // No platform-specific Instant representation or configurable TTL needed.
        let (mut low, mut high) = (0_u64, u64::MAX);
        while low < high {
            let mid = low + (high - low) / 2 + 1;
            if now.checked_add(Duration::from_secs(mid)).is_some() {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        let near_limit = now.checked_add(Duration::from_secs(low)).unwrap();
        assert!(near_limit.checked_add(PLAN_TTL).is_none());
        let mut registry = Registry::new();
        let token = registry.insert("db", 1, 10, now).unwrap();
        assert_eq!(
            registry.insert("db", 2, 20, near_limit),
            Err(InsertError::ClockOverflow)
        );
        assert_eq!(registry.pending.len(), 1);
        assert_eq!(registry.take(&token, "db", 1, now), Ok(10));
    }

    #[test]
    fn replacement_at_capacity_preserves_other_infobases() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let old = registry.insert("a", 1, 1, now).unwrap();
        let b = registry.insert("b", 1, 2, now).unwrap();
        let c = registry.insert("c", 1, 3, now).unwrap();
        let d = registry.insert("d", 1, 4, now).unwrap();
        let replacement = registry.insert("a", 1, 5, now).unwrap();
        assert_eq!(registry.pending.len(), MAX_PENDING);
        assert_eq!(registry.take(&old, "a", 1, now), Err(TakeError::Missing));
        for (token, target, value) in [(b, "b", 2), (c, "c", 3), (d, "d", 4), (replacement, "a", 5)]
        {
            assert_eq!(registry.take(&token, target, 1, now), Ok(value));
        }
    }

    #[test]
    fn fifth_distinct_plan_clears_all_previous_tokens() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let old: Vec<_> = ["a", "b", "c", "d"]
            .into_iter()
            .map(|target| (target, registry.insert(target, 1, 10, now).unwrap()))
            .collect();
        let fifth = registry.insert("e", 1, 20, now).unwrap();
        assert_eq!(registry.pending.len(), 1);
        for (target, token) in old {
            assert_eq!(
                registry.take(&token, target, 1, now),
                Err(TakeError::Missing)
            );
        }
        assert_eq!(registry.take(&fifth, "e", 1, now), Ok(20));
    }

    #[test]
    fn reconnect_and_shutdown_clear_contract() {
        let now = Instant::now();
        let mut registry = Registry::new();
        let reconnect = registry.insert("db", 1, 1, now).unwrap();
        registry.clear(); // Existing reconnect owner hook.
        assert_eq!(
            registry.take(&reconnect, "db", 2, now),
            Err(TakeError::Missing)
        );
        let shutdown = registry.insert("db", 2, 2, now).unwrap();
        registry.clear(); // Existing shutdown owner hook.
        assert_eq!(
            registry.take(&shutdown, "db", 2, now),
            Err(TakeError::Missing)
        );
        registry.clear();
        assert!(registry.pending.is_empty());
    }

    #[test]
    fn refusal_and_replacement_drop_owned_payloads() {
        use std::{cell::Cell, rc::Rc};
        struct Payload(Rc<Cell<usize>>);
        impl Drop for Payload {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let now = Instant::now();
        let mut registry = Registry::new();
        let old = registry
            .insert("db", 1, Payload(drops.clone()), now)
            .unwrap();
        let token = registry
            .insert("db", 1, Payload(drops.clone()), now)
            .unwrap();
        assert_eq!(drops.get(), 1);
        assert!(matches!(
            registry.take(&old, "db", 1, now),
            Err(TakeError::Missing)
        ));
        assert!(matches!(
            registry.take(&token, "db", 2, now),
            Err(TakeError::EpochChanged)
        ));
        assert_eq!(drops.get(), 2);
        registry.clear();
        assert_eq!(drops.get(), 2);
    }
}
