//! Event dispatch result types

use crate::ListenerError;

/// Result of event dispatch.
///
/// Contains information about the success or failure of event dispatch,
/// including any errors that occurred during listener execution.
#[derive(Debug)]
#[must_use = "DispatchResult carries listener errors that will be silently dropped if ignored"]
pub struct DispatchResult {
    results: Vec<Result<(), ListenerError>>,
    blocked: bool,
    listener_count: usize,
}

impl DispatchResult {
    pub(crate) fn new(results: Vec<Result<(), ListenerError>>) -> Self {
        let listener_count = results.len();
        Self {
            results,
            blocked: false,
            listener_count,
        }
    }

    pub(crate) fn blocked() -> Self {
        Self {
            results: Vec::new(),
            blocked: true,
            listener_count: 0,
        }
    }

    /// Check if the event was blocked by middleware.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Get the total number of listeners that were called.
    #[must_use]
    pub fn listener_count(&self) -> usize {
        self.listener_count
    }

    /// Get the number of successful handlers.
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_ok()).count()
    }

    /// Get the number of failed handlers.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_err()).count()
    }

    /// Borrow every error produced by failing listeners, in dispatch order.
    #[must_use]
    pub fn errors(&self) -> Vec<&ListenerError> {
        self.results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .collect()
    }

    /// Check if all handlers succeeded.
    ///
    /// Returns `false` if any listener errored or if the event was
    /// blocked by middleware.
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        !self.blocked && self.results.iter().all(|r| r.is_ok())
    }

    /// Check if any handlers failed.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.results.iter().any(|r| r.is_err())
    }
}
