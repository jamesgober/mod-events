//! Main event dispatcher implementation

use crate::error::panic_payload_to_listener_error;
use crate::metrics::EventMetricsCounters;
use crate::middleware::{self, MiddlewareManager};
use crate::type_id_map::TypeIdMap;
use crate::{
    DispatchResult, Event, EventMetadata, ListenerError, ListenerId, ListenerWrapper, Priority,
};
use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "async")]
use crate::AsyncListenerWrapper;

/// Listeners for one event type, in descending priority order (FIFO
/// within equal priority).
///
/// Copy-on-write: dispatch clones the `Arc` under the registry's read
/// lock and runs the listeners after releasing it, so no lock is held
/// while user code runs. Writers use `Arc::make_mut`, which mutates in
/// place when no dispatch holds a snapshot and clones the vector
/// otherwise.
type ListenerList = Arc<Vec<ListenerWrapper>>;

#[cfg(feature = "async")]
type AsyncListenerList = Arc<Vec<AsyncListenerWrapper>>;

/// High-performance event dispatcher
///
/// The main component of the Mod Events system. Thread-safe and optimized
/// for high-performance event dispatch with minimal overhead.
///
/// # Re-entrancy and concurrent changes
///
/// No internal lock is held while listeners or middleware run. Listeners
/// and middleware may therefore call back into the same dispatcher:
/// dispatch or emit other events, subscribe, unsubscribe (including
/// themselves), add or clear middleware, or call [`Self::clear`].
///
/// Each dispatch works from the listener list and middleware chain as
/// they were when it started. A listener added during a dispatch first
/// runs on the next dispatch, and a listener removed during a dispatch
/// (by another listener or by another thread) may still be invoked by
/// dispatches that were already in progress when it was removed.
///
/// # Example
///
/// ```rust
/// use mod_events::{EventDispatcher, Event};
///
/// #[derive(Debug, Clone)]
/// struct MyEvent {
///     message: String,
/// }
///
/// impl Event for MyEvent {
///     fn as_any(&self) -> &dyn std::any::Any {
///         self
///     }
/// }
///
/// let dispatcher = EventDispatcher::new();
///
/// dispatcher.on(|event: &MyEvent| {
///     println!("Received: {}", event.message);
/// });
///
/// dispatcher.emit(MyEvent {
///     message: "Hello, World!".to_string(),
/// });
/// ```
pub struct EventDispatcher {
    listeners: RwLock<TypeIdMap<ListenerList>>,
    #[cfg(feature = "async")]
    async_listeners: RwLock<TypeIdMap<AsyncListenerList>>,
    next_id: AtomicUsize,
    // Per-event-type counters. The dispatch hot path records into them
    // under the map's read lock (atomics only, no user code); the write
    // lock is taken once per event type to insert the entry. The `Arc`
    // lets `counters_for` hand out a handle that outlives the guard.
    metrics: RwLock<TypeIdMap<Arc<EventMetricsCounters>>>,
    middleware: RwLock<MiddlewareManager>,
    // `true` while the middleware chain is non-empty. Lets dispatch skip
    // the middleware lock entirely in the common no-middleware case.
    // Only written while the `middleware` write lock is held.
    has_middleware: AtomicBool,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            listeners: RwLock::new(TypeIdMap::default()),
            #[cfg(feature = "async")]
            async_listeners: RwLock::new(TypeIdMap::default()),
            next_id: AtomicUsize::new(0),
            metrics: RwLock::new(TypeIdMap::default()),
            middleware: RwLock::new(MiddlewareManager::new()),
            has_middleware: AtomicBool::new(false),
        }
    }

    /// Subscribe to an event with a closure that can return errors.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{EventDispatcher, Event};
    ///
    /// #[derive(Debug, Clone)]
    /// struct MyEvent {
    ///     message: String,
    /// }
    ///
    /// impl Event for MyEvent {
    ///     fn as_any(&self) -> &dyn std::any::Any {
    ///         self
    ///     }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.subscribe(|event: &MyEvent| {
    ///     if event.message.is_empty() {
    ///         return Err("Message cannot be empty".into());
    ///     }
    ///     println!("Message: {}", event.message);
    ///     Ok(())
    /// });
    /// ```
    pub fn subscribe<T, F>(&self, listener: F) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Result<(), ListenerError> + Send + Sync + 'static,
    {
        self.subscribe_with_priority(listener, Priority::Normal)
    }

    /// Subscribe to an event with a specific priority.
    pub fn subscribe_with_priority<T, F>(&self, listener: F, priority: Priority) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Result<(), ListenerError> + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let wrapper = ListenerWrapper::new(listener, priority, id);

        {
            let mut listeners = self.listeners.write();
            let event_listeners = Arc::make_mut(listeners.entry(type_id).or_default());
            // Binary insertion preserves descending-priority order in O(n)
            // (one shift), avoiding the O(n log n) full re-sort the
            // previous push + sort_by_key combination performed.
            // `partition_point` finds the first index whose listener has
            // a strictly lower priority than the new one, which is also
            // where to insert. Equal-priority listeners run in
            // registration order (FIFO).
            let pos = event_listeners.partition_point(|existing| existing.priority >= priority);
            event_listeners.insert(pos, wrapper);
        }

        // Make sure a metrics entry exists for this type so `metrics()`
        // can report it even before the first dispatch.
        let _counters = self.counters_for::<T>();

        ListenerId::new(id, type_id)
    }

    /// Subscribe to an event with simple closure (no error handling).
    ///
    /// This is the most convenient method for simple event handling.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{EventDispatcher, Event};
    ///
    /// #[derive(Debug, Clone)]
    /// struct MyEvent {
    ///     message: String,
    /// }
    ///
    /// impl Event for MyEvent {
    ///     fn as_any(&self) -> &dyn std::any::Any {
    ///         self
    ///     }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.on(|event: &MyEvent| {
    ///     println!("Received: {}", event.message);
    /// });
    /// ```
    pub fn on<T, F>(&self, listener: F) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.subscribe(move |event: &T| {
            listener(event);
            Ok(())
        })
    }

    /// Subscribe an async listener at [`Priority::Normal`] (requires the
    /// `async` feature).
    ///
    /// The listener receives a borrowed event and returns a future that
    /// resolves to `Result<(), ListenerError>`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "async")]
    /// # {
    /// use mod_events::{Event, EventDispatcher};
    ///
    /// #[derive(Debug, Clone)]
    /// struct EmailSent { to: String }
    ///
    /// impl Event for EmailSent {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.subscribe_async(|event: &EmailSent| {
    ///     let to = event.to.clone();
    ///     async move {
    ///         println!("delivered to {}", to);
    ///         Ok(())
    ///     }
    /// });
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub fn subscribe_async<T, F, Fut>(&self, listener: F) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), ListenerError>> + Send + 'static,
    {
        self.subscribe_async_with_priority(listener, Priority::Normal)
    }

    /// Subscribe an async listener at a specific priority (requires the
    /// `async` feature).
    ///
    /// Higher-priority listeners are awaited first within a single
    /// [`Self::dispatch_async`] call.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "async")]
    /// # {
    /// use mod_events::{Event, EventDispatcher, Priority};
    ///
    /// #[derive(Debug, Clone)]
    /// struct EmailSent { to: String }
    ///
    /// impl Event for EmailSent {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.subscribe_async_with_priority(
    ///     |_event: &EmailSent| async move {
    ///         // logged before any other listener
    ///         Ok(())
    ///     },
    ///     Priority::High,
    /// );
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub fn subscribe_async_with_priority<T, F, Fut>(
        &self,
        listener: F,
        priority: Priority,
    ) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), ListenerError>> + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let wrapper = AsyncListenerWrapper::new(listener, priority, id);

        {
            let mut async_listeners = self.async_listeners.write();
            let event_listeners = Arc::make_mut(async_listeners.entry(type_id).or_default());
            // Binary insertion preserves descending-priority order in O(n).
            // See `subscribe_with_priority` for the rationale.
            let pos = event_listeners.partition_point(|existing| existing.priority >= priority);
            event_listeners.insert(pos, wrapper);
        }

        // Make sure a metrics entry exists for this type so `metrics()`
        // can report it even before the first dispatch.
        let _counters = self.counters_for::<T>();

        ListenerId::new(id, type_id)
    }

    /// Dispatch an event synchronously.
    ///
    /// Runs every listener registered through [`Self::on`],
    /// [`Self::subscribe`], or [`Self::subscribe_with_priority`] for `T`,
    /// in descending priority order, and returns a [`DispatchResult`]
    /// containing per-listener outcomes. Async listeners are not invoked
    /// here; they only run through `dispatch_async`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{EventDispatcher, Event};
    ///
    /// #[derive(Debug, Clone)]
    /// struct MyEvent {
    ///     message: String,
    /// }
    ///
    /// impl Event for MyEvent {
    ///     fn as_any(&self) -> &dyn std::any::Any {
    ///         self
    ///     }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// let result = dispatcher.dispatch(MyEvent {
    ///     message: "Hello".to_string(),
    /// });
    ///
    /// if result.all_succeeded() {
    ///     println!("All listeners handled the event successfully");
    /// }
    /// ```
    pub fn dispatch<T: Event>(&self, event: T) -> DispatchResult {
        // Update metrics
        self.update_metrics(&event);

        // Check middleware
        if !self.check_middleware(&event) {
            return DispatchResult::blocked();
        }

        let Some(event_listeners) = self.listener_snapshot::<T>() else {
            return DispatchResult::new(0, Vec::new());
        };
        let event_any: &dyn Any = &event;

        // Most dispatches succeed. `errors` stays empty (and
        // unallocated: `Vec::new()` does not allocate) on the success
        // path; we only push when a listener returns `Err` or panics.
        let mut errors: Vec<ListenerError> = Vec::new();

        for listener in event_listeners.iter() {
            // `catch_unwind` keeps a panicking listener from unwinding
            // into the caller. Panics are converted into the same
            // `ListenerError` shape a well-behaved listener would have
            // returned, so the caller's `DispatchResult::errors()` is
            // the single place to look for failures. No registry lock
            // is held here, so a panic cannot leave one held.
            match catch_unwind(AssertUnwindSafe(|| (listener.handler)(event_any))) {
                Ok(Ok(())) => {}
                Ok(Err(err)) => errors.push(err),
                Err(payload) => errors.push(panic_payload_to_listener_error(payload)),
            }
        }

        DispatchResult::new(event_listeners.len(), errors)
    }

    /// Dispatch an event asynchronously and await every async listener
    /// in priority order (requires the `async` feature).
    ///
    /// Listeners are awaited **sequentially** in descending priority
    /// order. This preserves the priority contract — a high-priority
    /// listener completes (or errors) before the next listener is
    /// polled. Concurrent execution would lose ordering. If you want
    /// true concurrent execution, drive `spawn` (e.g. `tokio::spawn`,
    /// `async_std::task::spawn`) from inside each listener and return
    /// `Ok(())` immediately.
    ///
    /// Each listener call, and the future it returns, is wrapped in
    /// `catch_unwind`, mirroring the panic-safety guarantee of the sync
    /// [`Self::dispatch`] path. A panic while the listener builds its
    /// future or during `.await` becomes a `ListenerError` in
    /// [`DispatchResult::errors`] with the prefix
    /// `"listener panicked: "`. Subsequent listeners still run.
    ///
    /// Sync listeners registered via [`Self::on`] / [`Self::subscribe`]
    /// are not invoked here; only listeners registered through
    /// [`Self::subscribe_async`] / [`Self::subscribe_async_with_priority`]
    /// are awaited.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "async")]
    /// # async fn doc_example() {
    /// use mod_events::{Event, EventDispatcher};
    ///
    /// #[derive(Debug, Clone)]
    /// struct OrderShipped { order_id: u64 }
    ///
    /// impl Event for OrderShipped {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.subscribe_async(|event: &OrderShipped| {
    ///     let id = event.order_id;
    ///     async move {
    ///         println!("notifying customer about order {}", id);
    ///         Ok(())
    ///     }
    /// });
    ///
    /// let result = dispatcher
    ///     .dispatch_async(OrderShipped { order_id: 42 })
    ///     .await;
    /// assert!(result.all_succeeded());
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub async fn dispatch_async<T: Event>(&self, event: T) -> DispatchResult {
        // Update metrics
        self.update_metrics(&event);

        // Check middleware
        if !self.check_middleware(&event) {
            return DispatchResult::blocked();
        }

        // Snapshot the listener list (one reference-count increment, no
        // allocation) so no lock is held across await points.
        let Some(handlers) = self.async_listener_snapshot::<T>() else {
            return DispatchResult::new(0, Vec::new());
        };
        let event_any: &dyn Any = &event;

        // Wrap each listener in `catch_unwind` so a panicking listener
        // becomes a `ListenerError` in `DispatchResult` rather than
        // unwinding the dispatching task. Mirrors the panic-safety
        // wrapping on the sync `dispatch` path. Two places can panic:
        // the listener closure itself, which runs synchronously to build
        // the future, and the future while it is polled. Both are
        // covered.
        //
        // `AssertUnwindSafe` is required because the listener future
        // closes over arbitrary user state that may not implement
        // `UnwindSafe`. The contract is: if a listener panics, the
        // dispatcher catches it and reports it; the listener author is
        // responsible for not leaving captured state in a broken
        // condition before panicking. Same contract as the sync path.
        use futures_util::future::FutureExt;

        // `errors` stays empty (and unallocated) on the success path.
        let mut errors: Vec<ListenerError> = Vec::new();
        for listener in handlers.iter() {
            let future = match catch_unwind(AssertUnwindSafe(|| (listener.handler)(event_any))) {
                Ok(future) => future,
                Err(payload) => {
                    errors.push(panic_payload_to_listener_error(payload));
                    continue;
                }
            };
            match AssertUnwindSafe(future).catch_unwind().await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => errors.push(err),
                Err(payload) => errors.push(panic_payload_to_listener_error(payload)),
            }
        }

        DispatchResult::new(handlers.len(), errors)
    }

    /// Fire and forget — dispatch without inspecting the result.
    ///
    /// Use this when listeners' success or failure is not actionable at
    /// the call site (logging, fanout to passive observers). The errors
    /// returned by failing listeners are discarded; if you need them,
    /// call [`Self::dispatch`] instead.
    ///
    /// Like [`Self::dispatch`], only sync listeners are invoked.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{EventDispatcher, Event};
    ///
    /// #[derive(Debug, Clone)]
    /// struct MyEvent {
    ///     message: String,
    /// }
    ///
    /// impl Event for MyEvent {
    ///     fn as_any(&self) -> &dyn std::any::Any {
    ///         self
    ///     }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.emit(MyEvent {
    ///     message: "Fire and forget".to_string(),
    /// });
    /// ```
    pub fn emit<T: Event>(&self, event: T) {
        // Update metrics
        self.update_metrics(&event);

        // Check middleware
        if !self.check_middleware(&event) {
            return;
        }

        let Some(event_listeners) = self.listener_snapshot::<T>() else {
            return;
        };
        let event_any: &dyn Any = &event;

        for listener in event_listeners.iter() {
            // Same panic-safety contract as `dispatch`, but the outcome
            // is intentionally discarded: `emit` is fire-and-forget and
            // never builds a `DispatchResult`.
            let _ = catch_unwind(AssertUnwindSafe(|| (listener.handler)(event_any)));
        }
    }

    /// Add middleware that can block events.
    ///
    /// Middleware functions receive events and return `true` to allow
    /// processing or `false` to block the event.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{EventDispatcher, Event};
    ///
    /// let dispatcher = EventDispatcher::new();
    /// dispatcher.add_middleware(|event: &dyn Event| {
    ///     println!("Processing event: {}", event.event_name());
    ///     true // Allow all events
    /// });
    /// ```
    pub fn add_middleware<F>(&self, middleware: F)
    where
        F: Fn(&dyn Event) -> bool + Send + Sync + 'static,
    {
        let mut chain = self.middleware.write();
        chain.add(middleware);
        self.has_middleware.store(true, Ordering::Release);
    }

    /// Remove a previously registered listener.
    ///
    /// Returns `true` if the listener was found and removed, `false`
    /// if no listener with that id was registered (already removed,
    /// never registered, or registered against a different event type).
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{Event, EventDispatcher};
    ///
    /// #[derive(Debug, Clone)]
    /// struct Tick;
    ///
    /// impl Event for Tick {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// let id = dispatcher.on(|_: &Tick| {});
    /// assert!(dispatcher.unsubscribe(id));
    /// // Subsequent removals of the same id return false.
    /// assert!(!dispatcher.unsubscribe(id));
    /// ```
    pub fn unsubscribe(&self, listener_id: ListenerId) -> bool {
        // Try sync listeners first. The removed wrapper is moved out of
        // the locked block and dropped after the guard is released, so a
        // listener whose captured state calls back into the dispatcher on
        // drop cannot deadlock against the registry lock.
        let removed = {
            let mut listeners = self.listeners.write();
            listeners
                .get_mut(&listener_id.type_id)
                .and_then(|event_listeners| {
                    let pos = event_listeners
                        .iter()
                        .position(|l| l.id == listener_id.id)?;
                    Some(Arc::make_mut(event_listeners).remove(pos))
                })
        };
        if removed.is_some() {
            return true;
        }

        // Try async listeners
        #[cfg(feature = "async")]
        {
            let removed = {
                let mut async_listeners = self.async_listeners.write();
                async_listeners
                    .get_mut(&listener_id.type_id)
                    .and_then(|event_listeners| {
                        let pos = event_listeners
                            .iter()
                            .position(|l| l.id == listener_id.id)?;
                        Some(Arc::make_mut(event_listeners).remove(pos))
                    })
            };
            if removed.is_some() {
                return true;
            }
        }

        false
    }

    /// Get the total number of listeners (sync + async, when the
    /// `async` feature is enabled) registered for an event type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{Event, EventDispatcher};
    ///
    /// #[derive(Debug, Clone)]
    /// struct Tick;
    ///
    /// impl Event for Tick {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// assert_eq!(dispatcher.listener_count::<Tick>(), 0);
    /// let _ = dispatcher.on(|_: &Tick| {});
    /// let _ = dispatcher.on(|_: &Tick| {});
    /// assert_eq!(dispatcher.listener_count::<Tick>(), 2);
    /// ```
    #[must_use]
    pub fn listener_count<T: Event + 'static>(&self) -> usize {
        let type_id = TypeId::of::<T>();
        let sync_count = self
            .listeners
            .read()
            .get(&type_id)
            .map(|list| list.len())
            .unwrap_or(0);

        #[cfg(feature = "async")]
        let async_count = self
            .async_listeners
            .read()
            .get(&type_id)
            .map(|list| list.len())
            .unwrap_or(0);

        #[cfg(not(feature = "async"))]
        let async_count = 0;

        sync_count + async_count
    }

    /// Get a snapshot of per-event-type [`EventMetadata`] keyed by
    /// `TypeId`.
    ///
    /// The returned map is a fresh snapshot. Subsequent dispatches do
    /// not mutate it, but the snapshot may be slightly behind the live
    /// counters because `dispatch_count` is read independently of
    /// `last_dispatch`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{Event, EventDispatcher};
    /// use std::any::TypeId;
    ///
    /// #[derive(Debug, Clone)]
    /// struct Tick;
    ///
    /// impl Event for Tick {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// let _ = dispatcher.on(|_: &Tick| {});
    /// dispatcher.emit(Tick);
    /// dispatcher.emit(Tick);
    ///
    /// let snapshot = dispatcher.metrics();
    /// let meta = snapshot.get(&TypeId::of::<Tick>()).unwrap();
    /// assert_eq!(meta.dispatch_count, 2);
    /// ```
    #[must_use]
    pub fn metrics(&self) -> HashMap<TypeId, EventMetadata> {
        // Snapshot the metric counters first, then enrich each entry
        // with the live listener count derived from the registry. This
        // guarantees `listener_count` cannot drift because it is never
        // stored — it is computed at the moment of observation.
        let counters_map = self.metrics.read();
        let listeners_map = self.listeners.read();
        #[cfg(feature = "async")]
        let async_listeners_map = self.async_listeners.read();

        counters_map
            .iter()
            .map(|(type_id, counters)| {
                let mut snap = counters.snapshot();
                let sync_count = listeners_map
                    .get(type_id)
                    .map(|list| list.len())
                    .unwrap_or(0);
                #[cfg(feature = "async")]
                let async_count = async_listeners_map
                    .get(type_id)
                    .map(|list| list.len())
                    .unwrap_or(0);
                #[cfg(not(feature = "async"))]
                let async_count = 0;
                snap.listener_count = sync_count + async_count;
                (*type_id, snap)
            })
            .collect()
    }

    /// Drop every registered listener, both sync and async.
    ///
    /// Middleware and accumulated metrics are unaffected. Use
    /// [`Self::clear_middleware`] if you also need to drop the
    /// middleware chain.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mod_events::{Event, EventDispatcher};
    ///
    /// #[derive(Debug, Clone)]
    /// struct Tick;
    ///
    /// impl Event for Tick {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let dispatcher = EventDispatcher::new();
    /// let _ = dispatcher.on(|_: &Tick| {});
    /// dispatcher.clear();
    /// assert_eq!(dispatcher.listener_count::<Tick>(), 0);
    /// ```
    pub fn clear(&self) {
        // Swap the maps out under the lock and drop the old contents
        // after the guard is released; see `unsubscribe`.
        let _removed = std::mem::take(&mut *self.listeners.write());

        #[cfg(feature = "async")]
        let _removed_async = std::mem::take(&mut *self.async_listeners.write());
    }

    /// Drop every registered middleware function.
    ///
    /// Listeners and accumulated metrics are unaffected. Useful in
    /// test setup/teardown when you want to reset the middleware
    /// chain between cases without rebuilding the dispatcher.
    pub fn clear_middleware(&self) {
        // Released after the write guard is dropped; see `unsubscribe`.
        let _removed = {
            let mut chain = self.middleware.write();
            self.has_middleware.store(false, Ordering::Release);
            chain.take()
        };
    }

    /// Hot-path metric update. The fast path records the dispatch under
    /// the map's read lock without cloning the per-type `Arc` (recording
    /// is a few atomic operations and runs no user code). Only the first
    /// dispatch of a new event type takes the write lock.
    #[inline]
    fn update_metrics<T: Event>(&self, _event: &T) {
        if let Some(counters) = self.metrics.read().get(&TypeId::of::<T>()) {
            counters.record_dispatch();
            return;
        }
        self.counters_for::<T>().record_dispatch();
    }

    /// Snapshot the sync listeners registered for `T`. The read guard is
    /// a temporary dropped at the end of the statement, so the caller
    /// runs listeners with no registry lock held.
    #[inline]
    fn listener_snapshot<T: Event>(&self) -> Option<ListenerList> {
        self.listeners
            .read()
            .get(&TypeId::of::<T>())
            .filter(|list| !list.is_empty())
            .cloned()
    }

    /// Async counterpart of [`Self::listener_snapshot`].
    #[cfg(feature = "async")]
    #[inline]
    fn async_listener_snapshot<T: Event>(&self) -> Option<AsyncListenerList> {
        self.async_listeners
            .read()
            .get(&TypeId::of::<T>())
            .filter(|list| !list.is_empty())
            .cloned()
    }

    /// Look up (or create) the per-type counters. The fast path holds
    /// only a read lock; the slow path promotes to a write lock and
    /// double-checks the entry to avoid a torn-creation race.
    #[inline]
    fn counters_for<T: Event + 'static>(&self) -> Arc<EventMetricsCounters> {
        let type_id = TypeId::of::<T>();

        if let Some(existing) = self.metrics.read().get(&type_id) {
            return Arc::clone(existing);
        }

        let mut metrics = self.metrics.write();
        Arc::clone(
            metrics
                .entry(type_id)
                .or_insert_with(|| Arc::new(EventMetricsCounters::new::<T>())),
        )
    }

    /// Run the middleware chain. The chain is snapshotted under the read
    /// lock and executed after the guard is released, so middleware may
    /// call back into the dispatcher.
    #[inline]
    fn check_middleware(&self, event: &dyn Event) -> bool {
        if !self.has_middleware.load(Ordering::Acquire) {
            return true;
        }
        let chain = self.middleware.read().snapshot();
        match chain {
            None => true,
            Some(chain) => middleware::process(&chain, event),
        }
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
