//! Regression tests for the 1.2.0 fixes: a `Send` `dispatch_async`
//! future, `dispatch_async_all` running sync and async listeners
//! together, panicking middleware caught by the dispatcher, and listener
//! ids that do not match across dispatchers.

use mod_events::{Event, EventDispatcher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "async")]
use mod_events::Priority;
#[cfg(feature = "async")]
use std::sync::Mutex;

// The payload is read only through `Debug`, by the middleware tests.
#[derive(Debug, Clone)]
struct Ev(#[allow(dead_code)] u64);

impl Event for Ev {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn counter() -> (Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let c = Arc::new(AtomicUsize::new(0));
    (Arc::clone(&c), c)
}

// ---------------------------------------------------------------- Send

#[cfg(feature = "async")]
fn assert_send<T: Send>(_: &T) {}

/// Compile-time check: before 1.2.0 this did not build, because the
/// future held a `&dyn Any` (not `Send`) across an await point.
#[cfg(feature = "async")]
#[test]
fn test_dispatch_async_future_is_send() {
    let d = EventDispatcher::new();
    let fut = d.dispatch_async(Ev(1));
    assert_send(&fut);
    let fut_all = d.dispatch_async_all(Ev(2));
    assert_send(&fut_all);
    drop(fut);
    drop(fut_all);
}

#[cfg(feature = "async")]
#[test]
fn test_dispatch_async_runs_inside_tokio_spawn_on_multi_thread_runtime() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let d = Arc::new(EventDispatcher::new());
    let (hits, seen) = counter();
    let _id = d.subscribe_async(move |_: &Ev| {
        let hits = Arc::clone(&hits);
        async move {
            tokio::task::yield_now().await;
            let _ = hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });
    let d2 = Arc::clone(&d);
    let result = rt
        .block_on(async move {
            tokio::spawn(async move {
                let a = d2.dispatch_async(Ev(1)).await;
                let b = d2.dispatch_async_all(Ev(2)).await;
                (a.listener_count(), b.listener_count())
            })
            .await
        })
        .unwrap();
    assert_eq!(result, (1, 1));
    assert_eq!(seen.load(Ordering::SeqCst), 2);
}

// --------------------------------------------------- dispatch_async_all

#[cfg(feature = "async")]
#[tokio::test]
async fn test_dispatch_async_all_runs_sync_and_async_listeners() {
    let d = EventDispatcher::new();
    let (s, sync_runs) = counter();
    let (a, async_runs) = counter();
    let _ = d.on(move |_: &Ev| {
        let _ = s.fetch_add(1, Ordering::SeqCst);
    });
    let _ = d.subscribe_async(move |_: &Ev| {
        let a = Arc::clone(&a);
        async move {
            let _ = a.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });

    let result = d.dispatch_async_all(Ev(1)).await;
    assert_eq!(sync_runs.load(Ordering::SeqCst), 1);
    assert_eq!(async_runs.load(Ordering::SeqCst), 1);
    assert_eq!(result.listener_count(), d.listener_count::<Ev>());
    assert_eq!(result.listener_count(), 2);
    assert!(result.all_succeeded());

    // The existing methods keep their 1.x behaviour.
    let r = d.dispatch(Ev(2));
    assert_eq!(r.listener_count(), 1);
    let r = d.dispatch_async(Ev(3)).await;
    assert_eq!(r.listener_count(), 1);
    assert_eq!(sync_runs.load(Ordering::SeqCst), 2);
    assert_eq!(async_runs.load(Ordering::SeqCst), 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_dispatch_async_all_merges_in_priority_then_registration_order() {
    let d = EventDispatcher::new();
    let order = Arc::new(Mutex::new(Vec::new()));

    let push_sync = |name: &'static str, p: Priority| {
        let order = Arc::clone(&order);
        let _ = d.subscribe_with_priority(
            move |_: &Ev| {
                order.lock().unwrap().push(name);
                Ok(())
            },
            p,
        );
    };
    let push_async = |name: &'static str, p: Priority| {
        let order = Arc::clone(&order);
        let _ = d.subscribe_async_with_priority(
            move |_: &Ev| {
                let order = Arc::clone(&order);
                async move {
                    order.lock().unwrap().push(name);
                    Ok(())
                }
            },
            p,
        );
    };

    push_sync("s-normal-1", Priority::Normal);
    push_async("a-normal-2", Priority::Normal);
    push_async("a-high", Priority::High);
    push_sync("s-normal-3", Priority::Normal);
    push_sync("s-low", Priority::Low);
    push_async("a-critical", Priority::Critical);

    let r = d.dispatch_async_all(Ev(0)).await;
    assert_eq!(r.listener_count(), 6);
    assert_eq!(
        *order.lock().unwrap(),
        vec![
            "a-critical",
            "a-high",
            "s-normal-1",
            "a-normal-2",
            "s-normal-3",
            "s-low"
        ]
    );
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_dispatch_async_all_catches_panics_of_both_kinds() {
    let d = EventDispatcher::new();
    let (s, after) = counter();
    let _ = d.on(|_: &Ev| panic!("sync boom"));
    let _ = d.subscribe_async(|_: &Ev| async { panic!("async boom") });
    let _ = d.on(move |_: &Ev| {
        let _ = s.fetch_add(1, Ordering::SeqCst);
    });
    let r = d.dispatch_async_all(Ev(0)).await;
    assert_eq!(r.listener_count(), 3);
    assert_eq!(r.error_count(), 2);
    assert_eq!(after.load(Ordering::SeqCst), 1);
}

// ------------------------------------------------------ middleware panic

fn panicking_dispatcher() -> (EventDispatcher, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let d = EventDispatcher::new();
    let (l, listener_runs) = counter();
    let (m, later_middleware_runs) = counter();
    let _ = d.on(move |_: &Ev| {
        let _ = l.fetch_add(1, Ordering::SeqCst);
    });
    d.add_middleware(|e: &dyn Event| {
        if format!("{e:?}").contains("13") {
            panic!("filter bug");
        }
        true
    });
    d.add_middleware(move |_: &dyn Event| {
        let _ = m.fetch_add(1, Ordering::SeqCst);
        true
    });
    (d, listener_runs, later_middleware_runs)
}

#[test]
fn test_panicking_middleware_does_not_unwind_into_emit() {
    let (d, listener_runs, later) = panicking_dispatcher();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.emit(Ev(13))));
    assert!(outcome.is_ok(), "middleware panic escaped emit");
    assert_eq!(listener_runs.load(Ordering::SeqCst), 0);
    assert_eq!(later.load(Ordering::SeqCst), 0);

    // The dispatcher keeps working for events the middleware accepts.
    d.emit(Ev(1));
    assert_eq!(listener_runs.load(Ordering::SeqCst), 1);
}

#[test]
fn test_panicking_middleware_blocks_dispatch_and_reports_error() {
    let (d, listener_runs, later) = panicking_dispatcher();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.dispatch(Ev(13))));
    let result = outcome.expect("middleware panic escaped dispatch");
    assert!(result.is_blocked());
    assert!(!result.all_succeeded());
    assert_eq!(result.listener_count(), 0);
    assert_eq!(result.error_count(), 1);
    let msg = result.errors()[0].to_string();
    assert!(msg.starts_with("middleware panicked: "), "{msg}");
    assert!(msg.contains("filter bug"), "{msg}");
    assert_eq!(listener_runs.load(Ordering::SeqCst), 0);
    assert_eq!(later.load(Ordering::SeqCst), 0);

    // A plain block still carries no error.
    d.clear_middleware();
    d.add_middleware(|_: &dyn Event| false);
    let r = d.dispatch(Ev(1));
    assert!(r.is_blocked());
    assert_eq!(r.error_count(), 0);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_panicking_middleware_blocks_async_dispatch() {
    let (d, listener_runs, _later) = panicking_dispatcher();
    let r = d.dispatch_async(Ev(13)).await;
    assert!(r.is_blocked());
    assert_eq!(r.error_count(), 1);
    let r = d.dispatch_async_all(Ev(13)).await;
    assert!(r.is_blocked());
    assert_eq!(r.error_count(), 1);
    assert_eq!(listener_runs.load(Ordering::SeqCst), 0);
}

// ------------------------------------------------------------ listener ids

#[test]
fn test_listener_id_from_other_dispatcher_does_not_remove_listener() {
    let a = EventDispatcher::new();
    let b = EventDispatcher::new();
    let id_a = a.on(|_: &Ev| {});
    let (h, hits) = counter();
    let _id_b = b.on(move |_: &Ev| {
        let _ = h.fetch_add(1, Ordering::SeqCst);
    });
    assert!(!b.unsubscribe(id_a));
    b.emit(Ev(0));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(a.unsubscribe(id_a));
}
