//! Re-entrancy regression tests.
//!
//! Listeners and middleware are arbitrary user code, and calling back
//! into the dispatcher from inside them is a normal event-system
//! pattern: a one-shot listener that unsubscribes itself, a listener
//! that registers a follow-up listener, a listener that emits a derived
//! event. None of these may deadlock.
//!
//! Each scenario runs on a worker thread and the test waits on a
//! channel with a timeout, so a regression shows up as a test failure
//! instead of a hung test binary.

use mod_events::prelude::*;
use mod_events::ListenerId;
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
struct Ping;

impl Event for Ping {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct Pong;

impl Event for Pong {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const TIMEOUT: Duration = Duration::from_secs(10);

/// Run `scenario` on a worker thread and fail the test if it does not
/// finish within [`TIMEOUT`].
fn run_with_timeout<F>(what: &str, scenario: F)
where
    F: FnOnce() + Send + 'static,
{
    let (done_tx, done_rx) = mpsc::channel();
    let _worker = thread::spawn(move || {
        scenario();
        let _ = done_tx.send(());
    });
    match done_rx.recv_timeout(TIMEOUT) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{what}: dispatcher deadlocked (no progress after {TIMEOUT:?})")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{what}: scenario thread panicked")
        }
    }
}

#[test]
fn test_listener_unsubscribing_itself_during_dispatch_does_not_deadlock() {
    run_with_timeout("self-unsubscribe", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let own_id = Arc::new(Mutex::new(None::<ListenerId>));

        let id = {
            let dispatcher_in = Arc::downgrade(&dispatcher);
            let calls = calls.clone();
            let own_id = own_id.clone();
            dispatcher.on(move |_: &Ping| {
                let _ = calls.fetch_add(1, Ordering::SeqCst);
                let id = own_id.lock().unwrap().take();
                if let (Some(d), Some(id)) = (dispatcher_in.upgrade(), id) {
                    assert!(d.unsubscribe(id));
                }
            })
        };
        *own_id.lock().unwrap() = Some(id);

        let result = dispatcher.dispatch(Ping);
        assert!(result.all_succeeded());
        assert_eq!(result.listener_count(), 1);
        assert_eq!(dispatcher.listener_count::<Ping>(), 0);

        // One-shot: the second dispatch must not reach the listener.
        dispatcher.emit(Ping);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn test_listener_subscribing_during_dispatch_takes_effect_on_next_dispatch() {
    run_with_timeout("subscribe from listener", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let late_calls = Arc::new(AtomicUsize::new(0));
        let registered = Arc::new(AtomicBool::new(false));

        {
            let dispatcher_in = Arc::downgrade(&dispatcher);
            let late_calls = late_calls.clone();
            let registered = registered.clone();
            let _id = dispatcher.on(move |_: &Ping| {
                if registered.swap(true, Ordering::SeqCst) {
                    return;
                }
                if let Some(d) = dispatcher_in.upgrade() {
                    let late_calls = late_calls.clone();
                    let _late = d.on(move |_: &Ping| {
                        let _ = late_calls.fetch_add(1, Ordering::SeqCst);
                    });
                }
            });
        }

        // The listener added mid-dispatch is not part of this dispatch.
        let first = dispatcher.dispatch(Ping);
        assert_eq!(first.listener_count(), 1);
        assert_eq!(late_calls.load(Ordering::SeqCst), 0);

        // It is part of every later one.
        let second = dispatcher.dispatch(Ping);
        assert_eq!(second.listener_count(), 2);
        assert_eq!(late_calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn test_listener_clearing_dispatcher_during_dispatch_does_not_deadlock() {
    run_with_timeout("clear from listener", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let dispatcher_in = Arc::downgrade(&dispatcher);
        let _id = dispatcher.on(move |_: &Ping| {
            if let Some(d) = dispatcher_in.upgrade() {
                d.clear();
            }
        });

        let result = dispatcher.dispatch(Ping);
        assert!(result.all_succeeded());
        assert_eq!(dispatcher.listener_count::<Ping>(), 0);
    });
}

#[test]
fn test_nested_dispatch_with_concurrent_subscriber_does_not_deadlock() {
    // A listener for `Ping` emits a derived `Pong`. While the `Ping`
    // listener is running, a second thread calls `subscribe`, which
    // queues for the registry's write lock. A fair reader-writer lock
    // makes a second read acquisition on the dispatching thread wait
    // behind that queued writer, while the writer waits for the first
    // read guard to be released: a deadlock, unless dispatch holds no
    // registry lock while user code runs.
    run_with_timeout("nested dispatch vs concurrent subscribe", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let pongs = Arc::new(AtomicUsize::new(0));

        {
            let pongs = pongs.clone();
            let _id = dispatcher.on(move |_: &Pong| {
                let _ = pongs.fetch_add(1, Ordering::SeqCst);
            });
        }

        let (writer_go_tx, writer_go_rx) = mpsc::channel::<()>();
        let writer = {
            let dispatcher = dispatcher.clone();
            thread::spawn(move || {
                writer_go_rx.recv().unwrap();
                let _id = dispatcher.on(|_: &Ping| {});
            })
        };

        {
            let dispatcher_in = Arc::downgrade(&dispatcher);
            let writer_go_tx = Mutex::new(Some(writer_go_tx));
            let _id = dispatcher.on(move |_: &Ping| {
                if let Some(tx) = writer_go_tx.lock().unwrap().take() {
                    tx.send(()).unwrap();
                    // Give the writer time to park on the write lock.
                    thread::sleep(Duration::from_millis(100));
                }
                if let Some(d) = dispatcher_in.upgrade() {
                    d.emit(Pong);
                }
            });
        }

        let result = dispatcher.dispatch(Ping);
        assert!(result.all_succeeded());
        writer.join().unwrap();
        assert_eq!(pongs.load(Ordering::SeqCst), 1);
        assert_eq!(dispatcher.listener_count::<Ping>(), 2);
    });
}

#[test]
fn test_middleware_registering_middleware_during_dispatch_does_not_deadlock() {
    run_with_timeout("add_middleware from middleware", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let installed = Arc::new(AtomicBool::new(false));
        let second_calls = Arc::new(AtomicUsize::new(0));

        {
            let dispatcher_in = Arc::downgrade(&dispatcher);
            let installed = installed.clone();
            let second_calls = second_calls.clone();
            dispatcher.add_middleware(move |_: &dyn Event| {
                if !installed.swap(true, Ordering::SeqCst) {
                    if let Some(d) = dispatcher_in.upgrade() {
                        let second_calls = second_calls.clone();
                        d.add_middleware(move |_: &dyn Event| {
                            let _ = second_calls.fetch_add(1, Ordering::SeqCst);
                            true
                        });
                    }
                }
                true
            });
        }
        let _id = dispatcher.on(|_: &Ping| {});

        let first = dispatcher.dispatch(Ping);
        assert!(!first.is_blocked());
        assert_eq!(second_calls.load(Ordering::SeqCst), 0);

        let second = dispatcher.dispatch(Ping);
        assert!(!second.is_blocked());
        assert_eq!(second_calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn test_middleware_dispatching_from_inside_middleware_does_not_deadlock() {
    run_with_timeout("dispatch from middleware", || {
        let dispatcher = Arc::new(EventDispatcher::new());
        let pongs = Arc::new(AtomicUsize::new(0));
        {
            let pongs = pongs.clone();
            let _id = dispatcher.on(move |_: &Pong| {
                let _ = pongs.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let dispatcher_in = Arc::downgrade(&dispatcher);
            dispatcher.add_middleware(move |event: &dyn Event| {
                if event.as_any().is::<Ping>() {
                    if let Some(d) = dispatcher_in.upgrade() {
                        d.emit(Pong);
                        // A middleware may also mutate the chain.
                        d.clear_middleware();
                    }
                }
                true
            });
        }

        dispatcher.emit(Ping);
        assert_eq!(pongs.load(Ordering::SeqCst), 1);
    });
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_listener_unsubscribing_itself_during_dispatch_async_completes() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let own_id = Arc::new(Mutex::new(None::<ListenerId>));

    let id = {
        let dispatcher_in = Arc::downgrade(&dispatcher);
        let calls = calls.clone();
        let own_id = own_id.clone();
        dispatcher.subscribe_async(move |_: &Ping| {
            let _ = calls.fetch_add(1, Ordering::SeqCst);
            let id = own_id.lock().unwrap().take();
            if let (Some(d), Some(id)) = (dispatcher_in.upgrade(), id) {
                assert!(d.unsubscribe(id));
            }
            async { Ok(()) }
        })
    };
    *own_id.lock().unwrap() = Some(id);

    let result = tokio::time::timeout(TIMEOUT, dispatcher.dispatch_async(Ping))
        .await
        .expect("dispatch_async deadlocked");
    assert!(result.all_succeeded());
    let again = tokio::time::timeout(TIMEOUT, dispatcher.dispatch_async(Ping))
        .await
        .expect("dispatch_async deadlocked");
    assert_eq!(again.listener_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
