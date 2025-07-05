//! Integration tests for mod-events

use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Test events
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CounterEvent {
    value: i32,
}

impl Event for CounterEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_basic_event_dispatch() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    dispatcher.on(move |event: &TestEvent| {
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
fn test_multiple_listeners() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let counter1 = counter.clone();
    let counter2 = counter.clone();
    let counter3 = counter.clone();

    dispatcher.on(move |_: &TestEvent| {
        counter1.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.on(move |_: &TestEvent| {
        counter2.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.on(move |_: &TestEvent| {
        counter3.fetch_add(1, Ordering::SeqCst);
    });

    let result = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "multi".to_string(),
    });

    assert!(result.all_succeeded());
    assert_eq!(result.success_count(), 3);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn test_priority_ordering() {
    let dispatcher = EventDispatcher::new();
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let order1 = order.clone();
    let order2 = order.clone();
    let order3 = order.clone();

    dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order1.lock().unwrap().push(1);
            Ok(())
        },
        Priority::Low,
    );

    dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order2.lock().unwrap().push(2);
            Ok(())
        },
        Priority::High,
    );

    dispatcher.subscribe_with_priority(
        move |_: &TestEvent| {
            order3.lock().unwrap().push(3);
            Ok(())
        },
        Priority::Normal,
    );

    dispatcher.dispatch(TestEvent {
        id: 1,
        message: "priority".to_string(),
    });

    let final_order = order.lock().unwrap();
    assert_eq!(*final_order, vec![2, 3, 1]); // High, Normal, Low
}

#[test]
fn test_error_handling() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    // Working listener
    dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // Failing listener
    dispatcher.subscribe(|event: &TestEvent| {
        if event.id == 999 {
            Err("Test error".into())
        } else {
            Ok(())
        }
    });

    // Test success case
    let result1 = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "success".to_string(),
    });

    assert_eq!(result1.success_count(), 2);
    assert_eq!(result1.error_count(), 0);
    assert!(result1.all_succeeded());

    // Test error case
    let result2 = dispatcher.dispatch(TestEvent {
        id: 999,
        message: "error".to_string(),
    });

    assert_eq!(result2.success_count(), 1);
    assert_eq!(result2.error_count(), 1);
    assert!(!result2.all_succeeded());
    assert!(result2.has_errors());
}

#[test]
fn test_middleware_filtering() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    // Add middleware that blocks events with id = 999
    dispatcher.add_middleware(|event: &dyn Event| {
        if let Some(test_event) = event.as_any().downcast_ref::<TestEvent>() {
            test_event.id != 999
        } else {
            true
        }
    });

    dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // This should be blocked
    let result1 = dispatcher.dispatch(TestEvent {
        id: 999,
        message: "blocked".to_string(),
    });

    // This should go through
    let result2 = dispatcher.dispatch(TestEvent {
        id: 1,
        message: "allowed".to_string(),
    });

    assert!(result1.is_blocked());
    assert!(result2.all_succeeded());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_unsubscribe() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let listener_id = dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // Dispatch - should increment
    dispatcher.dispatch(TestEvent {
        id: 1,
        message: "test".to_string(),
    });

    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Unsubscribe
    assert!(dispatcher.unsubscribe(listener_id));

    // Dispatch again - should not increment
    dispatcher.dispatch(TestEvent {
        id: 2,
        message: "test2".to_string(),
    });

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_listener_count() {
    let dispatcher = EventDispatcher::new();

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 0);

    let _id1 = dispatcher.on(|_: &TestEvent| {});
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 1);

    let _id2 = dispatcher.on(|_: &TestEvent| {});
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 2);

    let id3 = dispatcher.on(|_: &TestEvent| {});
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 3);

    dispatcher.unsubscribe(id3);
    assert_eq!(dispatcher.listener_count::<TestEvent>(), 2);
}

#[test]
fn test_metrics() {
    let dispatcher = EventDispatcher::new();

    dispatcher.on(|_: &TestEvent| {});

    // Dispatch multiple times
    for i in 0..5 {
        dispatcher.dispatch(TestEvent {
            id: i,
            message: format!("test{i}"),
        });
    }

    let metrics = dispatcher.metrics();
    let test_event_metrics = metrics.get(&std::any::TypeId::of::<TestEvent>()).unwrap();
    assert_eq!(test_event_metrics.dispatch_count, 5);
    assert_eq!(
        test_event_metrics.event_name,
        "integration_tests::TestEvent"
    );
}

#[test]
fn test_fire_and_forget() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    dispatcher.on(move |_: &TestEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // emit() should dispatch the event but not return a result
    dispatcher.emit(TestEvent {
        id: 1,
        message: "fire and forget".to_string(),
    });

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_different_event_types() {
    let dispatcher = EventDispatcher::new();
    let test_counter = Arc::new(AtomicUsize::new(0));
    let counter_counter = Arc::new(AtomicUsize::new(0));

    let test_counter_clone = test_counter.clone();
    let counter_counter_clone = counter_counter.clone();

    dispatcher.on(move |_: &TestEvent| {
        test_counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.on(move |_: &CounterEvent| {
        counter_counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    dispatcher.emit(TestEvent {
        id: 1,
        message: "test".to_string(),
    });

    dispatcher.emit(CounterEvent { value: 42 });

    assert_eq!(test_counter.load(Ordering::SeqCst), 1);
    assert_eq!(counter_counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_clear() {
    let dispatcher = EventDispatcher::new();

    dispatcher.on(|_: &TestEvent| {});
    dispatcher.on(|_: &TestEvent| {});

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 2);

    dispatcher.clear();

    assert_eq!(dispatcher.listener_count::<TestEvent>(), 0);
}

#[cfg(feature = "async")]
mod async_tests {
    use super::*;

    #[tokio::test]
    async fn test_async_dispatch() {
        let dispatcher = EventDispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        dispatcher.subscribe_async(move |_: &TestEvent| {
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
    async fn test_async_priority() {
        let dispatcher = EventDispatcher::new();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let order1 = order.clone();
        let order2 = order.clone();

        dispatcher.subscribe_async_with_priority(
            move |_: &TestEvent| {
                let order = order1.clone();
                async move {
                    order.lock().unwrap().push(1);
                    Ok(())
                }
            },
            Priority::Low,
        );

        dispatcher.subscribe_async_with_priority(
            move |_: &TestEvent| {
                let order = order2.clone();
                async move {
                    order.lock().unwrap().push(2);
                    Ok(())
                }
            },
            Priority::High,
        );

        dispatcher
            .dispatch_async(TestEvent {
                id: 1,
                message: "async priority".to_string(),
            })
            .await;

        let final_order = order.lock().unwrap();
        assert_eq!(*final_order, vec![2, 1]); // High, Low
    }
}
