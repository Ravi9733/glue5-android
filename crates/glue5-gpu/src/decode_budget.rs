//! Byte accounting for CPU-side texture decode work.
//!
//! The renderer can use this tracker to keep compressed inputs, decoded BC
//! intermediates, and ETC2 outputs within one explicit memory budget.
//! Reservation is separate from job admission so callers can reserve before
//! moving source bytes into a worker closure.

#[derive(Debug)]
pub(crate) struct DecodeBudget {
    limit: usize,
    reserved: usize,
}

impl DecodeBudget {
    pub(crate) fn new(limit: usize) -> Self {
        Self { limit, reserved: 0 }
    }

    /// Reserve bytes without blocking. None means the request would exceed
    /// the configured budget or arithmetic capacity.
    pub(crate) fn try_reserve(&mut self, bytes: usize) -> Option<()> {
        let next = self.reserved.checked_add(bytes)?;
        // A single upload larger than the nominal budget may still proceed
        // when the budget is otherwise empty; otherwise the command ring
        // could remain permanently blocked on an unadmittable texture.
        if next > self.limit && self.reserved != 0 { return None; }
        self.reserved = next;
        Some(())
    }

    /// Release a prior reservation; reject bookkeeping mismatches.
    pub(crate) fn release(&mut self, bytes: usize) -> bool {
        let Some(next) = self.reserved.checked_sub(bytes) else { return false; };
        self.reserved = next;
        true
    }

    #[cfg(test)]
    pub(crate) fn reserved(&self) -> usize { self.reserved }
    #[cfg(test)]
    pub(crate) fn available(&self) -> usize { self.limit.saturating_sub(self.reserved) }
}

#[cfg(test)]
mod tests {
    use super::DecodeBudget;

    #[test]
    fn reservations_are_bounded_and_reusable() {
        let mut budget = DecodeBudget::new(100);
        assert_eq!(budget.try_reserve(60), Some(()));
        assert_eq!(budget.reserved(), 60);
        assert_eq!(budget.available(), 40);
        assert_eq!(budget.try_reserve(41), None);
        assert_eq!(budget.reserved(), 60);
        assert!(budget.release(60));
        assert_eq!(budget.try_reserve(100), Some(()));
        assert_eq!(budget.available(), 0);
    }

    #[test]
    fn oversized_job_is_admitted_only_when_budget_is_empty() {
        let mut budget = DecodeBudget::new(100);
        assert_eq!(budget.try_reserve(120), Some(()));
        assert_eq!(budget.available(), 0);
        assert_eq!(budget.try_reserve(1), None);
        assert!(budget.release(120));
        assert_eq!(budget.try_reserve(80), Some(()));
        assert_eq!(budget.try_reserve(120), None);
    }

    #[test]
    fn overflow_and_invalid_release_are_rejected() {
        let mut budget = DecodeBudget::new(usize::MAX);
        assert_eq!(budget.try_reserve(usize::MAX), Some(()));
        assert_eq!(budget.try_reserve(1), None);
        assert!(!budget.release(1));
        assert_eq!(budget.reserved(), usize::MAX);
        assert!(budget.release(usize::MAX));
        assert_eq!(budget.reserved(), 0);
    }
}
