//! Event dispatch metrics and monitoring

use crate::Event;
use std::any::TypeId;
use std::time::Instant;

/// Event metadata for debugging and monitoring
///
/// Contains information about event dispatch history and performance.
#[derive(Debug, Clone)]
pub struct EventMetadata {
    /// The name of the event type
    pub event_name: &'static str,
    /// Type ID of the event
    pub type_id: TypeId,
    /// Timestamp of the last dispatch
    pub last_dispatch: Instant,
    /// Total number of times this event has been dispatched
    pub dispatch_count: usize,
    /// Number of listeners currently subscribed to this event
    pub listener_count: usize,
}

impl EventMetadata {
    pub(crate) fn new<T: Event>() -> Self {
        Self {
            event_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            last_dispatch: Instant::now(),
            dispatch_count: 0,
            listener_count: 0,
        }
    }

    pub(crate) fn increment_dispatch(&mut self) {
        self.dispatch_count += 1;
        self.last_dispatch = Instant::now();
    }

    pub(crate) fn update_listener_count(&mut self, count: usize) {
        self.listener_count = count;
    }

    /// Get the time since the last dispatch
    pub fn time_since_last_dispatch(&self) -> std::time::Duration {
        self.last_dispatch.elapsed()
    }
}
