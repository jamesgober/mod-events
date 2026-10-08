//! Event dispatch metrics and monitoring.
//!
//! The dispatcher records per-event-type counts (dispatches, listeners)
//! and a last-dispatch timestamp. The hot dispatch path only performs
//! atomic read-modify-write operations on the per-type counters; it
//! never takes a write lock on the aggregate metrics map and never
//! takes a mutex. [`EventDispatcher::metrics`](crate::EventDispatcher::metrics)
//! returns an immutable [`EventMetadata`] snapshot per event type.

use crate::Event;
use std::any::TypeId;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Per-event-type counters held inside the dispatcher.
///
/// Every update is a single atomic operation. `std::time::Instant` has
/// no atomic representation, so the last-dispatch time is stored as the
/// number of nanoseconds since `created` (the moment this entry was
/// inserted) in an `AtomicU64` and turned back into an `Instant` when a
/// snapshot is taken. `fetch_max` keeps the stored value monotonic when
/// dispatches on different threads race, so a slower writer can never
/// move the timestamp backwards. A `u64` of nanoseconds covers roughly
/// 584 years of process uptime; longer values saturate.
///
/// `listener_count` deliberately lives outside this struct: it is
/// derived from the live listener vecs at snapshot time, so it can
/// never disagree with the actual registry. There is no atomic to
/// keep in sync across `subscribe`, `unsubscribe`, and `clear`.
pub(crate) struct EventMetricsCounters {
    pub(crate) event_name: &'static str,
    pub(crate) type_id: TypeId,
    pub(crate) dispatch_count: AtomicU64,
    created: Instant,
    last_dispatch_nanos: AtomicU64,
}

impl EventMetricsCounters {
    pub(crate) fn new<T: Event>() -> Self {
        Self {
            event_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            dispatch_count: AtomicU64::new(0),
            created: Instant::now(),
            last_dispatch_nanos: AtomicU64::new(0),
        }
    }

    #[inline]
    pub(crate) fn record_dispatch(&self) {
        let _previous = self.dispatch_count.fetch_add(1, Ordering::Relaxed);
        let nanos = u64::try_from(self.created.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let _previous = self.last_dispatch_nanos.fetch_max(nanos, Ordering::Relaxed);
    }

    /// Timestamp of the most recent dispatch, or of the entry's creation
    /// when nothing has been dispatched yet.
    fn last_dispatch(&self) -> Instant {
        let nanos = self.last_dispatch_nanos.load(Ordering::Relaxed);
        self.created
            .checked_add(Duration::from_nanos(nanos))
            .unwrap_or(self.created)
    }

    /// Snapshot the counters this struct owns. Listener count is not
    /// tracked here; the dispatcher fills it in from the live registry
    /// when assembling the public [`EventMetadata`].
    pub(crate) fn snapshot(&self) -> EventMetadata {
        EventMetadata {
            event_name: self.event_name,
            type_id: self.type_id,
            last_dispatch: self.last_dispatch(),
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
    ///
    /// When `dispatch_count` is `0` the event type has never been
    /// dispatched and this holds the time its metrics entry was created,
    /// which is the first `subscribe*` call for the type. Check
    /// `dispatch_count` before treating it as a dispatch time.
    pub last_dispatch: Instant,
    /// Total number of times this event type has been dispatched
    /// since the dispatcher was created.
    ///
    /// Counts every call to `dispatch`, `dispatch_async`, and `emit`,
    /// including calls that middleware then blocked and calls for which
    /// no listener was registered.
    pub dispatch_count: u64,
    /// Number of listeners (sync + async) registered for this event
    /// type at the moment the snapshot was taken.
    pub listener_count: usize,
}

impl EventMetadata {
    /// How long ago the most recent dispatch of this event type was.
    ///
    /// Measured from [`EventMetadata::last_dispatch`], so for an event
    /// type that has never been dispatched (`dispatch_count == 0`) this
    /// is the time since its metrics entry was created.
    #[must_use]
    pub fn time_since_last_dispatch(&self) -> std::time::Duration {
        self.last_dispatch.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Probe;

    impl Event for Probe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_snapshot_before_any_dispatch_reports_creation_time() {
        let counters = EventMetricsCounters::new::<Probe>();
        let snap = counters.snapshot();
        assert_eq!(snap.dispatch_count, 0);
        assert_eq!(snap.last_dispatch, counters.created);
    }

    #[test]
    fn test_record_dispatch_never_moves_timestamp_backwards() {
        let counters = EventMetricsCounters::new::<Probe>();
        counters.record_dispatch();
        let first = counters.snapshot().last_dispatch;
        // Simulate a racing writer that read the clock earlier and
        // stores a smaller offset after the newer one landed.
        let _previous = counters.last_dispatch_nanos.fetch_max(0, Ordering::Relaxed);
        assert_eq!(counters.snapshot().last_dispatch, first);
        std::thread::sleep(Duration::from_millis(2));
        counters.record_dispatch();
        let second = counters.snapshot();
        assert!(second.last_dispatch > first);
        assert_eq!(second.dispatch_count, 2);
    }
}
