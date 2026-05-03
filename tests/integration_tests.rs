//! Integration tests for mod-events.
//!
//! Test names follow the REPS convention `test_<subject>_<condition>_<expected>`.

use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct TestEvent {
    id: u64,
    message: String,
}

impl Event for TestEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct CounterEvent {
    value: i32,
}

impl Event for CounterEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct UnusedEvent;

impl Event for UnusedEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Core dispatch behavior
// ---------------------------------------------------------------------------

#[test]
fn test_dispatch_with_subscribed_listener_invokes_handler_once() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let _id = dispatcher.on(move |event: &TestEvent| {
        assert_eq!(event.id, 123);
        assert_eq!(event.message, "test");
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    let result = dispatcher.dispatch(TestEvent {
        id: 123,
        message: "test".to_string(),
    });

    assert!(result.all_succeeded());
    assert_eq!(result.success_count(), 1);
    assert_eq!(result.error_count(), 0);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_dispatch_with_multiple_listeners_invokes_each_once() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..3 {
        let counter_clone = counter.clone();
        let _id = dispatcher.on(move |_: &TestEvent| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
    }

    let result = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "multi".to_string(),
    });

    assert!(result.all_succeeded());
    assert_eq!(result.success_count(), 3);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn test_dispatch_with_mixed_priorities_executes_highest_first() {
    let dispatcher = EventDispatcher::new();
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let order_low = order.clone();
    let order_high = order.clone();
    let order_normal = order.clone();

    let _l = dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order_low.lock().unwrap().push(1);
            Ok(())
        },
        Priority::Low,
    );
    let _h = dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order_high.lock().unwrap().push(2);
            Ok(())
        },
        Priority::High,
    );
    let _n = dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order_normal.lock().unwrap().push(3);
            Ok(())
        },
        Priority::Normal,
    );

    dispatcher.emit(TestEvent {
        id: 1,
        message: "priority".to_string(),
    });

    assert_eq!(*order.lock().unwrap(), vec![2, 3, 1]);
}

#[test]
fn test_dispatch_with_equal_priorities_preserves_registration_order() {
    let dispatcher = EventDispatcher::new();
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    for i in 0..5 {
        let order_clone = order.clone();
        let _id = dispatcher.subscribe_with_priority(
            move |_: &TestEvent| {
                order_clone.lock().unwrap().push(i);
                Ok(())
            },
            Priority::Normal,
        );
    }

    dispatcher.emit(TestEvent {
        id: 1,
        message: "fifo".to_string(),
    });

    assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_dispatch_with_failing_listener_collects_error_without_stopping_dispatch() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let _ok = dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });
    let _err = dispatcher.subscribe(|event: &TestEvent| {
        if event.id == 999 {
            Err("Test error".into())
        } else {
            Ok(())
        }
    });

    let success = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "success".to_string(),
    });
    assert_eq!(success.success_count(), 2);
    assert_eq!(success.error_count(), 0);
    assert!(success.all_succeeded());

    let failure = dispatcher.dispatch(TestEvent {
        id: 999,
        message: "error".to_string(),
    });
    assert_eq!(failure.success_count(), 1);
    assert_eq!(failure.error_count(), 1);
    assert!(!failure.all_succeeded());
    assert!(failure.has_errors());
    assert_eq!(failure.errors().len(), 1);
    // Both listeners ran even though one returned `Err`.
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_dispatch_with_no_listeners_returns_all_succeeded() {
    let dispatcher = EventDispatcher::new();

    let result = dispatcher.dispatch(UnusedEvent);

    assert!(result.all_succeeded());
    assert_eq!(result.success_count(), 0);
    assert_eq!(result.error_count(), 0);
    assert!(!result.is_blocked());
    assert!(!result.has_errors());
}

#[test]
fn test_dispatch_routes_event_to_matching_listener_type_only() {
    let dispatcher = EventDispatcher::new();
    let test_counter = Arc::new(AtomicUsize::new(0));
    let counter_counter = Arc::new(AtomicUsize::new(0));

    let test_clone = test_counter.clone();
    let _ta = dispatcher.on(move |_: &TestEvent| {
        test_clone.fetch_add(1, Ordering::SeqCst);
    });
    let counter_clone = counter_counter.clone();
    let _ca = dispatcher.on(move |event: &CounterEvent| {
        assert_eq!(event.value, 42);
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.emit(TestEvent {
        id: 1,
        message: "test".to_string(),
    });
    dispatcher.emit(CounterEvent { value: 42 });

    assert_eq!(test_counter.load(Ordering::SeqCst), 1);
    assert_eq!(counter_counter.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Middleware behavior
// ---------------------------------------------------------------------------

#[test]
fn test_dispatch_with_blocking_middleware_returns_blocked() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    dispatcher.add_middleware(|event: &dyn Event| {
        if let Some(test_event) = event.as_any().downcast_ref::<TestEvent>() {
            test_event.id != 999
        } else {
            true
        }
    });

    let _id = dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    let blocked = dispatcher.dispatch(TestEvent {
        id: 999,
        message: "blocked".to_string(),
    });
    let allowed = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "allowed".to_string(),
    });

    assert!(blocked.is_blocked());
    assert!(!blocked.all_succeeded());
    assert!(allowed.all_succeeded());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_middleware_chain_executes_in_registration_order_and_short_circuits() {
    let dispatcher = EventDispatcher::new();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));

    let calls_a = calls.clone();
    dispatcher.add_middleware(move |_: &dyn Event| {
        calls_a.lock().unwrap().push("a");
        true
    });

    let calls_b = calls.clone();
    dispatcher.add_middleware(move |_: &dyn Event| {
        calls_b.lock().unwrap().push("b");
        false // short-circuit
    });

    let calls_c = calls.clone();
    dispatcher.add_middleware(move |_: &dyn Event| {
        calls_c.lock().unwrap().push("c");
        true
    });

    let result = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "chain".to_string(),
    });

    assert!(result.is_blocked());
    // `c` must NOT have run because `b` returned false.
    assert_eq!(*calls.lock().unwrap(), vec!["a", "b"]);
}

// ---------------------------------------------------------------------------
// Subscribe / unsubscribe / clear
// ---------------------------------------------------------------------------

#[test]
fn test_unsubscribe_after_dispatch_stops_subsequent_invocation() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let id = dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.emit(TestEvent {
        id: 1,
        message: "first".to_string(),
    });
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    assert!(dispatcher.unsubscribe(id));

    dispatcher.emit(TestEvent {
        id: 2,
        message: "second".to_string(),
    });
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_unsubscribe_with_unknown_id_returns_false() {
    let dispatcher = EventDispatcher::new();
    let id = dispatcher.on(|_: &TestEvent| {});
    assert!(dispatcher.unsubscribe(id));
    // Same id removed twice — second call must be a no-op.
    assert!(!dispatcher.unsubscribe(id));
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_unsubscribe_async_listener_removes_it() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let id = dispatcher.subscribe_async(move |_: &TestEvent| {
        let counter = counter_clone.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });

    let _r1 = dispatcher
        .dispatch_async(TestEvent {
            id: 1,
            message: "before".to_string(),
        })
        .await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    assert!(dispatcher.unsubscribe(id));

    let _r2 = dispatcher
        .dispatch_async(TestEvent {
            id: 2,
            message: "after".to_string(),
        })
        .await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_listener_count_reflects_register_and_remove() {
    let dispatcher = EventDispatcher::new();

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 0);

    let _id1 = dispatcher.on(|_: &TestEvent| {});
    let _id2 = dispatcher.on(|_: &TestEvent| {});
    let id3 = dispatcher.on(|_: &TestEvent| {});

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 3);

    assert!(dispatcher.unsubscribe(id3));
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 2);
}

#[test]
fn test_clear_drops_all_listeners() {
    let dispatcher = EventDispatcher::new();
    let _id1 = dispatcher.on(|_: &TestEvent| {});
    let _id2 = dispatcher.on(|_: &TestEvent| {});
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 2);

    dispatcher.clear();

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 0);
}

#[test]
fn test_subscribe_64_listeners_all_invoked_in_priority_order() {
    let dispatcher = EventDispatcher::new();
    let invocations = Arc::new(AtomicUsize::new(0));

    // 64 listeners: half High, half Low, registered High-then-Low. The
    // dispatcher should invoke all 32 High listeners before any Low one.
    for _ in 0..32 {
        let invocations = invocations.clone();
        let _id = dispatcher.subscribe_with_priority(
            move |_: &TestEvent| {
                let prior = invocations.fetch_add(1, Ordering::SeqCst);
                assert!(prior < 32, "Low listener ran before all Highs");
                Ok(())
            },
            Priority::High,
        );
    }
    for _ in 0..32 {
        let invocations = invocations.clone();
        let _id = dispatcher.subscribe_with_priority(
            move |_: &TestEvent| {
                let prior = invocations.fetch_add(1, Ordering::SeqCst);
                assert!(prior >= 32, "High listener ran out of order");
                Ok(())
            },
            Priority::Low,
        );
    }

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 64);

    let result = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "stress".to_string(),
    });

    assert_eq!(result.success_count(), 64);
    assert_eq!(invocations.load(Ordering::SeqCst), 64);
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

#[test]
fn test_metrics_record_dispatch_counts_per_event_type() {
    let dispatcher = EventDispatcher::new();
    let _id = dispatcher.on(|_: &TestEvent| {});

    for i in 0..5 {
        dispatcher.emit(TestEvent {
            id: i,
            message: format!("test{i}"),
        });
    }

    let metrics = dispatcher.metrics();
    let meta = metrics.get(&std::any::TypeId::of::<TestEvent>()).unwrap();
    assert_eq!(meta.dispatch_count, 5);
    assert_eq!(meta.event_name, "integration_tests::TestEvent");
    assert_eq!(meta.listener_count, 1);
}

#[test]
fn test_emit_invokes_listener_without_returning_result() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let _id = dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // emit() returns ()
    dispatcher.emit(TestEvent {
        id: 1,
        message: "fire and forget".to_string(),
    });

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Async tests
// ---------------------------------------------------------------------------

#[cfg(feature = "async")]
mod async_tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatch_async_awaits_subscribed_async_listener() {
        let dispatcher = EventDispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let _id = dispatcher.subscribe_async(move |_: &TestEvent| {
            let counter = counter_clone.clone();
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });

        let result = dispatcher
            .dispatch_async(TestEvent {
                id: 1,
                message: "async test".to_string(),
            })
            .await;

        assert!(result.all_succeeded());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_dispatch_async_executes_high_priority_listener_first() {
        let dispatcher = EventDispatcher::new();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let order_low = order.clone();
        let order_high = order.clone();

        let _low = dispatcher.subscribe_async_with_priority(
            move |_: &TestEvent| {
                let order = order_low.clone();
                async move {
                    order.lock().unwrap().push(1);
                    Ok(())
                }
            },
            Priority::Low,
        );

        let _high = dispatcher.subscribe_async_with_priority(
            move |_: &TestEvent| {
                let order = order_high.clone();
                async move {
                    order.lock().unwrap().push(2);
                    Ok(())
                }
            },
            Priority::High,
        );

        let _result = dispatcher
            .dispatch_async(TestEvent {
                id: 1,
                message: "async priority".to_string(),
            })
            .await;

        assert_eq!(*order.lock().unwrap(), vec![2, 1]);
    }
}
