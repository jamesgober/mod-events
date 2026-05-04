//! Event dispatch result types.

use crate::ListenerError;

/// Outcome of an event dispatch.
///
/// Carries the per-listener error list and aggregate counts. The
/// internal representation is *lazy*: a `DispatchResult` for a fully
/// successful dispatch holds an empty `Vec` (zero heap allocation
/// — `Vec::new` does not allocate until first `push`). Allocation
/// only happens when at least one listener returns `Err` or panics,
/// keeping the success path allocation-free.
#[derive(Debug)]
#[must_use = "DispatchResult carries listener errors that will be silently dropped if ignored"]
pub struct DispatchResult {
    /// Number of listeners that ran. Includes both successes and
    /// failures. Equal to `success_count + error_count` on the
    /// non-blocked path; equal to `0` if `blocked == true`.
    listener_count: usize,
    /// Errors produced by failing listeners, in dispatch order.
    /// Stays empty (and unallocated) when every listener succeeds.
    errors: Vec<ListenerError>,
    /// `true` iff the dispatch was halted by middleware before any
    /// listener ran.
    blocked: bool,
}

impl DispatchResult {
    /// Construct a result for a normal (non-blocked) dispatch.
    pub(crate) fn new(listener_count: usize, errors: Vec<ListenerError>) -> Self {
        Self {
            listener_count,
            errors,
            blocked: false,
        }
    }

    /// Construct a result for a dispatch that was halted by middleware.
    pub(crate) fn blocked() -> Self {
        Self {
            listener_count: 0,
            errors: Vec::new(),
            blocked: false,
        }
        .with_blocked()
    }

    fn with_blocked(mut self) -> Self {
        self.blocked = true;
        self
    }

    /// Whether the dispatch was halted by middleware.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Total number of listeners invoked (successes + failures).
    /// Returns `0` when the dispatch was blocked.
    #[must_use]
    pub fn listener_count(&self) -> usize {
        self.listener_count
    }

    /// Number of listeners that returned `Ok(())` (or completed
    /// without panicking).
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.listener_count.saturating_sub(self.errors.len())
    }

    /// Number of listeners that returned `Err(_)` or panicked. A
    /// panicking listener contributes one error with the message
    /// prefix `"listener panicked: "`.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Borrow every error produced by failing listeners, in dispatch order.
    #[must_use]
    pub fn errors(&self) -> &[ListenerError] {
        &self.errors
    }

    /// `true` iff the dispatch was not blocked and every listener
    /// returned `Ok(())`.
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        !self.blocked && self.errors.is_empty()
    }

    /// `true` iff at least one listener returned `Err(_)` or panicked.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}
