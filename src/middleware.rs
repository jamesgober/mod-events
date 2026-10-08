//! Middleware system for event processing.
//!
//! This module is `pub(crate)` for the 1.x line. Callers register
//! middleware through [`crate::EventDispatcher::add_middleware`] and
//! drop it through [`crate::EventDispatcher::clear_middleware`];
//! the internal manager type and its function-alias type are not part
//! of the public surface.

use crate::Event;
use std::sync::Arc;

/// Middleware function type stored internally by the dispatcher.
pub(crate) type MiddlewareFunction = Arc<dyn Fn(&dyn Event) -> bool + Send + Sync>;

/// Shared, immutable view of the middleware chain at one point in time.
pub(crate) type MiddlewareChain = Arc<Vec<MiddlewareFunction>>;

/// Middleware manager: internal storage backing
/// [`crate::EventDispatcher::add_middleware`] and friends.
///
/// The chain is copy-on-write. A dispatch takes a cheap [`Arc`] snapshot
/// under the dispatcher's read lock and runs the middleware after the
/// lock is released, so middleware may call back into the dispatcher
/// (dispatch another event, add or clear middleware) without
/// deadlocking. `add` mutates in place when no dispatch holds a
/// snapshot and clones the vector otherwise.
pub(crate) struct MiddlewareManager {
    middleware: MiddlewareChain,
}

impl std::fmt::Debug for MiddlewareManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiddlewareManager")
            .field("middleware_count", &self.middleware.len())
            .finish()
    }
}

impl MiddlewareManager {
    pub(crate) fn new() -> Self {
        Self {
            middleware: Arc::new(Vec::new()),
        }
    }

    /// Add middleware to the chain. Middleware is executed in
    /// registration order; if any middleware returns `false` the
    /// event is blocked.
    pub(crate) fn add<F>(&mut self, middleware: F)
    where
        F: Fn(&dyn Event) -> bool + Send + Sync + 'static,
    {
        Arc::make_mut(&mut self.middleware).push(Arc::new(middleware));
    }

    /// Snapshot of the current chain, or `None` when it is empty so the
    /// common no-middleware dispatch skips the reference-count update.
    #[inline]
    pub(crate) fn snapshot(&self) -> Option<MiddlewareChain> {
        if self.middleware.is_empty() {
            None
        } else {
            Some(Arc::clone(&self.middleware))
        }
    }

    /// Drop every registered middleware, returning the previous chain so
    /// the caller can release it after dropping its lock guard.
    pub(crate) fn take(&mut self) -> MiddlewareChain {
        std::mem::replace(&mut self.middleware, Arc::new(Vec::new()))
    }
}

/// Run every middleware in `chain` in order. Returns `true` if the
/// event should continue, `false` as soon as one middleware blocks it.
#[inline]
pub(crate) fn process(chain: &[MiddlewareFunction], event: &dyn Event) -> bool {
    chain.iter().all(|m| m(event))
}
