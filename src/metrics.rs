//! Event dispatch metrics and monitoring.
//!
//! The dispatcher records per-event-type counts (dispatches, listeners)
//! and a last-dispatch timestamp. The hot dispatch path increments
//! atomic counters and updates a tiny per-type [`parking_lot::Mutex`]
//! protecting the timestamp; it never takes a write lock on the
//! aggregate metrics map. [`EventDispatcher::metrics`](crate::EventDispatcher::metrics)
//! returns an immutable [`EventMetadata`] snapshot per event type.

use crate::Event;
use parking_lot::Mutex;
use std::any::TypeId;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Per-event-type counters held inside the dispatcher.
///
/// All increment paths are lock-free except the timestamp swap, which
/// uses a `parking_lot::Mutex<Instant>` because `std::time::Instant`
/// has no public atomic representation. The mutex critical section
/// stores a single `Instant` and never blocks on user code, so
/// contention is bounded by how often the same event fires.
///
/// `listener_count` deliberately lives outside this struct: it is
/// derived from the live listener vecs at snapshot time, so it can
/// never disagree with the actual registry — there is no atomic to
/// keep in sync across `subscribe`, `unsubscribe`, and `clear`.
pub(crate) struct EventMetricsCounters {
    pub(crate) event_name: &'static str,
    pub(crate) type_id: TypeId,
    pub(crate) dispatch_count: AtomicU64,
    pub(crate) last_dispatch: Mutex<Instant>,
}

impl EventMetricsCounters {
    pub(crate) fn new<T: Event>() -> Self {
        Self {
            event_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            dispatch_count: AtomicU64::new(0),
            last_dispatch: Mutex::new(Instant::now()),
        }
    }

    #[inline]
    pub(crate) fn record_dispatch(&self) {
        let _previous = self.dispatch_count.fetch_add(1, Ordering::Relaxed);
        *self.last_dispatch.lock() = Instant::now();
    }

    /// Snapshot the counters this struct owns. Listener count is not
    /// tracked here; the dispatcher fills it in from the live registry
    /// when assembling the public [`EventMetadata`].
    pub(crate) fn snapshot(&self) -> EventMetadata {
        EventMetadata {
            event_name: self.event_name,
            type_id: self.type_id,
            last_dispatch: *self.last_dispatch.lock(),
            dispatch_count: self.dispatch_count.load(Ordering::Relaxed),
            listener_count: 0,
        }
    }
}

/// Immutable snapshot of an event type's metrics.
///
/// Returned by [`crate::EventDispatcher::metrics`]. Each field reflects
/// the value at the moment the snapshot was taken; subsequent dispatches
/// do not mutate it.
///
/// Marked `#[non_exhaustive]` so future minor releases may add metric
/// fields (e.g. error counts, percentile latencies) without breaking
/// existing callers. External code must read fields by name, never
/// construct `EventMetadata` via struct-literal syntax. The struct is
/// only produced inside the crate.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EventMetadata {
    /// Fully qualified name of the event type, as reported by
    /// [`std::any::type_name`].
    pub event_name: &'static str,
    /// `TypeId` of the event type. Stable for a given type within a
    /// single process run.
    pub type_id: TypeId,
    /// Timestamp of the most recent dispatch of this event type.
    pub last_dispatch: Instant,
    /// Total number of times this event type has been dispatched
    /// since the dispatcher was created.
    pub dispatch_count: u64,
    /// Number of listeners (sync + async) registered for this event
    /// type at the moment the snapshot was taken.
    pub listener_count: usize,
}

impl EventMetadata {
    /// How long ago the most recent dispatch of this event type was.
    #[must_use]
    pub fn time_since_last_dispatch(&self) -> std::time::Duration {
        self.last_dispatch.elapsed()
    }
}
