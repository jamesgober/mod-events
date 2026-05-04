//! Property-based tests for the dispatcher.
//!
//! Each test in this file generates randomised inputs and asserts an
//! invariant the dispatcher must hold across every input. These
//! complement the deterministic integration tests in `integration_tests.rs`
//! by covering the input space the author would not think to write
//! by hand.
//!
//! REPS §Testing: "Algorithms that SHOULD hold for all inputs MUST
//! be covered by property-based tests using `proptest`."

use mod_events::prelude::*;
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct PropEvent {
    // Carries the dispatch index for properties that care about
    // monotonic identity. Listeners may or may not read it depending
    // on the property under test.
    #[allow(dead_code)]
    payload: u64,
}

impl Event for PropEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// `Priority` is not directly `Arbitrary`, so generate it via index.
fn arb_priority() -> impl Strategy<Value = Priority> {
    (0..6usize).prop_map(|n| match n {
        0 => Priority::Lowest,
        1 => Priority::Low,
        2 => Priority::Normal,
        3 => Priority::High,
        4 => Priority::Highest,
        _ => Priority::Critical,
    })
}

proptest! {
    /// Subscribing N listeners then dispatching one event must invoke
    /// every listener exactly once and return a `DispatchResult` whose
    /// `listener_count` equals N. Holds for any N in [0, 256].
    #[test]
    fn prop_dispatch_invokes_every_listener_exactly_once(
        n in 0_usize..256,
    ) {
        let dispatcher = EventDispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..n {
            let counter = counter.clone();
            let _id = dispatcher.on(move |_: &PropEvent| {
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }

        let result = dispatcher.dispatch(PropEvent { payload: 0 });

        prop_assert_eq!(result.listener_count(), n);
        prop_assert_eq!(result.success_count(), n);
        prop_assert_eq!(result.error_count(), 0);
        prop_assert_eq!(counter.load(Ordering::Relaxed), n);
        prop_assert!(result.all_succeeded());
        prop_assert!(!result.has_errors());
        prop_assert!(!result.is_blocked());
    }

    /// For any sequence of `(priority, sentinel)` registrations, the
    /// dispatch order observed by the listeners is descending by
    /// priority and FIFO within equal priority — the same ordering
    /// the dispatcher's documentation promises.
    #[test]
    fn prop_dispatch_respects_priority_then_fifo(
        ops in proptest::collection::vec(arb_priority(), 0..32),
    ) {
        let dispatcher = EventDispatcher::new();
        let observed: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        // Each listener pushes its registration index when invoked.
        for (idx, priority) in ops.iter().copied().enumerate() {
            let observed = observed.clone();
            let _id = dispatcher.subscribe_with_priority(
                move |_: &PropEvent| {
                    observed.lock().unwrap().push(idx);
                    Ok(())
                },
                priority,
            );
        }

        dispatcher.emit(PropEvent { payload: 0 });

        // Compute the expected order: stable-sort registration indices
        // by descending priority. Stable sort preserves FIFO within
        // equal priority — same semantic the dispatcher's binary
        // insertion gives.
        let mut expected: Vec<usize> = (0..ops.len()).collect();
        expected.sort_by_key(|&i| std::cmp::Reverse(ops[i]));

        let actual = observed.lock().unwrap().clone();
        prop_assert_eq!(actual, expected);
    }

    /// Subscribing N listeners and then unsubscribing M of them
    /// (M ≤ N) leaves exactly N − M listeners on the dispatcher,
    /// regardless of which subset of IDs is removed.
    #[test]
    fn prop_subscribe_then_unsubscribe_count_invariant(
        (n, removals) in (1_usize..64).prop_flat_map(|n| {
            (
                Just(n),
                proptest::collection::vec(0_usize..n, 0..n),
            )
        }),
    ) {
        let dispatcher = EventDispatcher::new();
        let mut ids = Vec::with_capacity(n);

        for _ in 0..n {
            ids.push(dispatcher.on(|_: &PropEvent| {}));
        }
        prop_assert_eq!(dispatcher.listener_count::<PropEvent>(), n);

        // Deduplicate the indices to remove (a real unsubscribe of an
        // already-removed id returns false and does not change the
        // count, which would skew the assertion).
        let mut unique_removals: Vec<usize> = removals;
        unique_removals.sort_unstable();
        unique_removals.dedup();

        let mut removed = 0_usize;
        for idx in unique_removals {
            if dispatcher.unsubscribe(ids[idx]) {
                removed += 1;
            }
        }

        prop_assert_eq!(dispatcher.listener_count::<PropEvent>(), n - removed);
    }

    /// Dispatching the same event N times increments the
    /// per-event-type `dispatch_count` metric by exactly N. Holds
    /// regardless of whether listeners exist.
    #[test]
    fn prop_dispatch_count_metric_matches_call_count(
        (listeners, dispatches) in (0_usize..16, 1_u64..256),
    ) {
        let dispatcher = EventDispatcher::new();
        let invocations = Arc::new(AtomicUsize::new(0));

        for _ in 0..listeners {
            let invocations = invocations.clone();
            let _id = dispatcher.on(move |_: &PropEvent| {
                invocations.fetch_add(1, Ordering::Relaxed);
            });
        }

        for i in 0..dispatches {
            dispatcher.emit(PropEvent { payload: i });
        }

        let snapshot = dispatcher.metrics();
        let meta = snapshot
            .get(&std::any::TypeId::of::<PropEvent>())
            .expect("PropEvent must have a metrics entry after dispatch");
        prop_assert_eq!(meta.dispatch_count, dispatches);
        prop_assert_eq!(meta.listener_count, listeners);
        prop_assert_eq!(
            invocations.load(Ordering::Relaxed) as u64,
            dispatches * (listeners as u64),
        );
    }

    /// A middleware chain blocks the dispatch iff *any* middleware in
    /// the chain returns false. Listeners run iff every middleware
    /// returned true. Order of the booleans does not change the
    /// outcome.
    #[test]
    fn prop_middleware_chain_blocks_iff_any_returns_false(
        chain in proptest::collection::vec(any::<bool>(), 0..16),
    ) {
        let dispatcher = EventDispatcher::new();
        let listener_ran = Arc::new(AtomicUsize::new(0));

        for &allow in &chain {
            // Each middleware closure captures its own `allow` flag.
            dispatcher.add_middleware(move |_event: &dyn Event| allow);
        }

        let listener_ran_clone = listener_ran.clone();
        let _id = dispatcher.on(move |_: &PropEvent| {
            listener_ran_clone.fetch_add(1, Ordering::Relaxed);
        });

        let result = dispatcher.dispatch(PropEvent { payload: 0 });
        let any_false = chain.iter().any(|&b| !b);

        if any_false {
            prop_assert!(result.is_blocked());
            prop_assert_eq!(result.listener_count(), 0);
            prop_assert_eq!(listener_ran.load(Ordering::Relaxed), 0);
        } else {
            prop_assert!(!result.is_blocked());
            prop_assert_eq!(result.listener_count(), 1);
            prop_assert_eq!(listener_ran.load(Ordering::Relaxed), 1);
        }
    }

    /// `clear` followed by `listener_count` always returns 0,
    /// regardless of how many listeners were registered or what their
    /// priorities were. Subsequent dispatches see zero listeners.
    #[test]
    fn prop_clear_drops_all_listeners(
        priorities in proptest::collection::vec(arb_priority(), 0..64),
    ) {
        let dispatcher = EventDispatcher::new();

        for priority in priorities.iter().copied() {
            let _id = dispatcher.subscribe_with_priority(
                |_: &PropEvent| Ok(()),
                priority,
            );
        }

        dispatcher.clear();

        prop_assert_eq!(dispatcher.listener_count::<PropEvent>(), 0);
        let result = dispatcher.dispatch(PropEvent { payload: 0 });
        prop_assert_eq!(result.listener_count(), 0);
        prop_assert!(result.all_succeeded());
    }
}
