//! Middleware system for event processing

use crate::Event;

/// Middleware function type
///
/// Middleware functions receive an event and return `true` to allow
/// the event to continue processing, or `false` to block it.
pub type MiddlewareFunction = Box<dyn Fn(&dyn Event) -> bool + Send + Sync>;

/// Middleware manager for event processing
///
/// Middleware allows you to intercept events before they reach listeners.
/// This is useful for logging, filtering, or transforming events.
pub struct MiddlewareManager {
    middleware: Vec<MiddlewareFunction>,
}

impl std::fmt::Debug for MiddlewareManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiddlewareManager")
            .field("middleware_count", &self.middleware.len())
            .finish()
    }
}

impl Default for MiddlewareManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareManager {
    /// Create a new middleware manager
    pub fn new() -> Self {
        Self {
            middleware: Vec::new(),
        }
    }

    /// Add middleware to the chain
    ///
    /// Middleware is executed in the order it was added.
    /// If any middleware returns `false`, the event is blocked.
    pub fn add<F>(&mut self, middleware: F)
    where
        F: Fn(&dyn Event) -> bool + Send + Sync + 'static,
    {
        self.middleware.push(Box::new(middleware));
    }

    /// Process an event through all middleware
    ///
    /// Returns `true` if the event should continue, `false` if blocked.
    pub fn process(&self, event: &dyn Event) -> bool {
        self.middleware.iter().all(|m| m(event))
    }

    /// Get the number of middleware functions
    pub fn count(&self) -> usize {
        self.middleware.len()
    }

    /// Clear all middleware
    pub fn clear(&mut self) {
        self.middleware.clear();
    }
}
