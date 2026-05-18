//! Middleware system for event processing.
//!
//! This module is `pub(crate)` for the 1.x line. Callers register
//! middleware through [`crate::EventDispatcher::add_middleware`] and
//! drop it through [`crate::EventDispatcher::clear_middleware`];
//! the internal manager type and its function-alias type are not part
//! of the public surface.

use crate::Event;

/// Middleware function type stored internally by the dispatcher.
pub(crate) type MiddlewareFunction = Box<dyn Fn(&dyn Event) -> bool + Send + Sync>;

/// Middleware manager — internal storage backing
/// [`crate::EventDispatcher::add_middleware`] and friends.
pub(crate) struct MiddlewareManager {
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
    pub(crate) fn new() -> Self {
        Self {
            middleware: Vec::new(),
        }
    }

    /// Add middleware to the chain. Middleware is executed in
    /// registration order; if any middleware returns `false` the
    /// event is blocked.
    pub(crate) fn add<F>(&mut self, middleware: F)
    where
        F: Fn(&dyn Event) -> bool + Send + Sync + 'static,
    {
        self.middleware.push(Box::new(middleware));
    }

    /// Process an event through every middleware. Returns `true` if
    /// the event should continue, `false` if blocked.
    #[inline]
    pub(crate) fn process(&self, event: &dyn Event) -> bool {
        self.middleware.iter().all(|m| m(event))
    }

    /// Drop every registered middleware.
    pub(crate) fn clear(&mut self) {
        self.middleware.clear();
    }
}
