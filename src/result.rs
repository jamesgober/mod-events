//! Event dispatch result types

/// Result of event dispatch
///
/// Contains information about the success or failure of event dispatch,
/// including any errors that occurred during listener execution.
#[derive(Debug)]
pub struct DispatchResult {
    results: Vec<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    blocked: bool,
    listener_count: usize,
}

impl DispatchResult {
    pub(crate) fn new(results: Vec<Result<(), Box<dyn std::error::Error + Send + Sync>>>) -> Self {
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

    /// Check if the event was blocked by middleware
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Get the total number of listeners that were called
    pub fn listener_count(&self) -> usize {
        self.listener_count
    }

    /// Get the number of successful handlers
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_ok()).count()
    }

    /// Get the number of failed handlers
    pub fn error_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_err()).count()
    }

    /// Get all errors that occurred during dispatch
    pub fn errors(&self) -> Vec<&(dyn std::error::Error + Send + Sync)> {
        self.results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .map(|e| e.as_ref())
            .collect()
    }

    /// Check if all handlers succeeded
    pub fn all_succeeded(&self) -> bool {
        !self.blocked && self.results.iter().all(|r| r.is_ok())
    }

    /// Check if any handlers failed
    pub fn has_errors(&self) -> bool {
        self.results.iter().any(|r| r.is_err())
    }
}
