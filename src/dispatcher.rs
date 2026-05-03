//! Main event dispatcher implementation

use crate::metrics::EventMetricsCounters;
use crate::{
    DispatchResult, Event, EventMetadata, ListenerError, ListenerId, ListenerWrapper,
    MiddlewareManager, Priority,
};
use parking_lot::RwLock;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "async")]
use crate::{AsyncEventResult, AsyncListenerWrapper};

#[cfg(feature = "async")]
type AsyncHandler = Arc<dyn for<'a> Fn(&'a dyn Event) -> AsyncEventResult<'a> + Send + Sync>;

/// High-performance event dispatcher
///
/// The main component of the Mod Events system. Thread-safe and optimized
/// for high-performance event dispatch with minimal overhead.
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
    listeners: Arc<RwLock<HashMap<TypeId, Vec<ListenerWrapper>>>>,
    #[cfg(feature = "async")]
    async_listeners: Arc<RwLock<HashMap<TypeId, Vec<AsyncListenerWrapper>>>>,
    next_id: AtomicUsize,
    // Per-event-type counters live behind an `Arc` so the dispatch hot
    // path can clone the counter pointer under a read lock and increment
    // its atomics without ever touching the outer map's write lock.
    metrics: Arc<RwLock<HashMap<TypeId, Arc<EventMetricsCounters>>>>,
    middleware: Arc<RwLock<MiddlewareManager>>,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "async")]
            async_listeners: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicUsize::new(0),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            middleware: Arc::new(RwLock::new(MiddlewareManager::new())),
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
            let event_listeners = listeners.entry(type_id).or_default();
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
            let event_listeners = async_listeners.entry(type_id).or_default();
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
    /// Returns a [`DispatchResult`] containing per-listener outcomes.
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

        let type_id = TypeId::of::<T>();
        let listeners = self.listeners.read();
        let mut results = Vec::new();

        if let Some(event_listeners) = listeners.get(&type_id) {
            results.reserve(event_listeners.len());
            for listener in event_listeners {
                results.push((listener.handler)(&event));
            }
        }

        DispatchResult::new(results)
    }

    /// Dispatch an event asynchronously and await every async listener
    /// in priority order (requires the `async` feature).
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

        let type_id = TypeId::of::<T>();

        // Collect cloned handlers without holding the lock across await points.
        let handlers: Vec<AsyncHandler> = {
            let async_listeners = self.async_listeners.read();
            async_listeners
                .get(&type_id)
                .map(|event_listeners| {
                    event_listeners
                        .iter()
                        .map(|listener| listener.handler.clone())
                        .collect()
                })
                .unwrap_or_default()
        };

        let mut results = Vec::with_capacity(handlers.len());
        for handler in handlers {
            results.push(handler(&event).await);
        }

        DispatchResult::new(results)
    }

    /// Fire and forget — dispatch without inspecting the result.
    ///
    /// Use this when listeners' success or failure is not actionable at
    /// the call site (logging, fanout to passive observers). The errors
    /// returned by failing listeners are discarded; if you need them,
    /// call [`Self::dispatch`] instead.
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
        // Intentional discard: emit is the documented fire-and-forget entry
        // point. Callers that need per-listener outcomes use `dispatch`.
        drop(self.dispatch(event));
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
        self.middleware.write().add(middleware);
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
        // Try sync listeners first
        {
            let mut listeners = self.listeners.write();
            if let Some(event_listeners) = listeners.get_mut(&listener_id.type_id) {
                if let Some(pos) = event_listeners.iter().position(|l| l.id == listener_id.id) {
                    let _removed = event_listeners.remove(pos);
                    return true;
                }
            }
        }

        // Try async listeners
        #[cfg(feature = "async")]
        {
            let mut async_listeners = self.async_listeners.write();
            if let Some(event_listeners) = async_listeners.get_mut(&listener_id.type_id) {
                if let Some(pos) = event_listeners.iter().position(|l| l.id == listener_id.id) {
                    let _removed = event_listeners.remove(pos);
                    return true;
                }
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
            .map(Vec::len)
            .unwrap_or(0);

        #[cfg(feature = "async")]
        let async_count = self
            .async_listeners
            .read()
            .get(&type_id)
            .map(Vec::len)
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
                let sync_count = listeners_map.get(type_id).map(Vec::len).unwrap_or(0);
                #[cfg(feature = "async")]
                let async_count = async_listeners_map.get(type_id).map(Vec::len).unwrap_or(0);
                #[cfg(not(feature = "async"))]
                let async_count = 0;
                snap.listener_count = sync_count + async_count;
                (*type_id, snap)
            })
            .collect()
    }

    /// Drop every registered listener, both sync and async.
    ///
    /// Middleware and accumulated metrics are unaffected.
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
        self.listeners.write().clear();

        #[cfg(feature = "async")]
        self.async_listeners.write().clear();
    }

    /// Hot-path metric update. Tries a read-only fast path first; only
    /// promotes to a write lock if the entry doesn't exist yet.
    fn update_metrics<T: Event>(&self, _event: &T) {
        let counters = self.counters_for::<T>();
        counters.record_dispatch();
    }

    /// Look up (or create) the per-type counters. The fast path holds
    /// only a read lock; the slow path promotes to a write lock and
    /// double-checks the entry to avoid a torn-creation race.
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

    fn check_middleware(&self, event: &dyn Event) -> bool {
        self.middleware.read().process(event)
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
