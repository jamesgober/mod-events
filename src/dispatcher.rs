//! Main event dispatcher implementation

use crate::{
    DispatchResult, Event, EventMetadata, ListenerId, ListenerWrapper, MiddlewareManager, Priority,
};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

#[cfg(feature = "async")]
use crate::AsyncListenerWrapper;
#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;

// Type aliases for complex types
#[cfg(feature = "async")]
type AsyncResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
#[cfg(feature = "async")]
type AsyncHandler = Arc<
    dyn for<'a> Fn(&'a dyn Event) -> Pin<Box<dyn Future<Output = AsyncResult> + Send + 'a>>
        + Send
        + Sync,
>;

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
    metrics: Arc<RwLock<HashMap<TypeId, EventMetadata>>>,
    middleware: Arc<RwLock<MiddlewareManager>>,
}

impl EventDispatcher {
    /// Create a new event dispatcher
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

    // Subscribe to an event with a closure that can return errors
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
    ///     // Handle event, can return errors
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
        F: Fn(&T) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
    {
        self.subscribe_with_priority(listener, Priority::Normal)
    }

    /// Subscribe to an event with a specific priority
    pub fn subscribe_with_priority<T, F>(&self, listener: F, priority: Priority) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let wrapper = ListenerWrapper::new(listener, priority, id);

        let mut listeners = self.listeners.write().unwrap();
        let event_listeners = listeners.entry(type_id).or_default();
        event_listeners.push(wrapper);

        // Sort by priority (highest first)
        event_listeners.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Update metrics
        drop(listeners); // Drop the lock before calling update_listener_count
        self.update_listener_count::<T>();

        ListenerId::new(id, type_id)
    }

    /// Subscribe to an event with simple closure (no error handling)
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

    /// Subscribe to an async event (requires "async" feature)
    #[cfg(feature = "async")]
    pub fn subscribe_async<T, F, Fut>(&self, listener: F) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        self.subscribe_async_with_priority(listener, Priority::Normal)
    }

    /// Subscribe to an async event with priority (requires "async" feature)
    #[cfg(feature = "async")]
    pub fn subscribe_async_with_priority<T, F, Fut>(
        &self,
        listener: F,
        priority: Priority,
    ) -> ListenerId
    where
        T: Event + 'static,
        F: Fn(&T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        let type_id = TypeId::of::<T>();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let wrapper = AsyncListenerWrapper::new(listener, priority, id);

        let mut async_listeners = self.async_listeners.write().unwrap();
        let event_listeners = async_listeners.entry(type_id).or_default();
        event_listeners.push(wrapper);

        // Sort by priority (highest first)
        event_listeners.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Update metrics
        drop(async_listeners); // Drop the lock before calling update_listener_count
        self.update_listener_count::<T>();

        ListenerId::new(id, type_id)
    }

    /// Dispatch an event synchronously
    ///
    /// Returns a `DispatchResult` containing information about the dispatch.
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
        let listeners = self.listeners.read().unwrap();
        let mut results = Vec::new();

        if let Some(event_listeners) = listeners.get(&type_id) {
            results.reserve(event_listeners.len());
            for listener in event_listeners {
                results.push((listener.handler)(&event));
            }
        }

        DispatchResult::new(results)
    }

    /// Dispatch an event asynchronously (requires "async" feature)
    #[cfg(feature = "async")]
    pub async fn dispatch_async<T: Event>(&self, event: T) -> DispatchResult {
        // Update metrics
        self.update_metrics(&event);

        // Check middleware
        if !self.check_middleware(&event) {
            return DispatchResult::blocked();
        }

        let type_id = TypeId::of::<T>();

        // Collect cloned handlers without holding the lock
        let handlers: Vec<AsyncHandler> = {
            let async_listeners = self.async_listeners.read().unwrap();
            if let Some(event_listeners) = async_listeners.get(&type_id) {
                event_listeners
                    .iter()
                    .map(|listener| listener.handler.clone())
                    .collect()
            } else {
                Vec::new()
            }
        }; // Lock is dropped here

        // Now execute all handlers without holding any locks
        let mut results = Vec::with_capacity(handlers.len());

        for handler in handlers {
            let future = handler(&event);
            results.push(future.await);
        }

        DispatchResult::new(results)
    }

    /// Fire and forget - dispatch without waiting for results
    ///
    /// This is the most efficient way to dispatch events when you don't
    /// need to check the results.
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
        let _ = self.dispatch(event);
    }

    /// Add middleware that can block events
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
        let mut middleware_manager = self.middleware.write().unwrap();
        middleware_manager.add(middleware);
    }

    /// Remove a listener
    ///
    /// Returns `true` if the listener was found and removed, `false` otherwise.
    pub fn unsubscribe(&self, listener_id: ListenerId) -> bool {
        // Try sync listeners first
        {
            let mut listeners = self.listeners.write().unwrap();
            if let Some(event_listeners) = listeners.get_mut(&listener_id.type_id) {
                if let Some(pos) = event_listeners.iter().position(|l| l.id == listener_id.id) {
                    event_listeners.remove(pos);
                    return true;
                }
            }
        }

        // Try async listeners
        #[cfg(feature = "async")]
        {
            let mut async_listeners = self.async_listeners.write().unwrap();
            if let Some(event_listeners) = async_listeners.get_mut(&listener_id.type_id) {
                if let Some(pos) = event_listeners.iter().position(|l| l.id == listener_id.id) {
                    event_listeners.remove(pos);
                    return true;
                }
            }
        }

        false
    }

    /// Get the number of listeners for an event type
    pub fn listener_count<T: Event + 'static>(&self) -> usize {
        let type_id = TypeId::of::<T>();
        let sync_count = self
            .listeners
            .read()
            .unwrap()
            .get(&type_id)
            .map(|v| v.len())
            .unwrap_or(0);

        #[cfg(feature = "async")]
        let async_count = self
            .async_listeners
            .read()
            .unwrap()
            .get(&type_id)
            .map(|v| v.len())
            .unwrap_or(0);

        #[cfg(not(feature = "async"))]
        let async_count = 0;

        sync_count + async_count
    }

    /// Get event metrics
    pub fn metrics(&self) -> HashMap<TypeId, EventMetadata> {
        self.metrics.read().unwrap().clone()
    }

    /// Clear all listeners
    pub fn clear(&self) {
        self.listeners.write().unwrap().clear();

        #[cfg(feature = "async")]
        self.async_listeners.write().unwrap().clear();
    }

    fn update_metrics<T: Event>(&self, _event: &T) {
        let mut metrics = self.metrics.write().unwrap();
        let type_id = TypeId::of::<T>();

        match metrics.get_mut(&type_id) {
            Some(meta) => {
                meta.increment_dispatch();
            }
            None => {
                let mut meta = EventMetadata::new::<T>();
                meta.increment_dispatch();
                metrics.insert(type_id, meta);
            }
        }
    }

    fn update_listener_count<T: Event + 'static>(&self) {
        let mut metrics = self.metrics.write().unwrap();
        let type_id = TypeId::of::<T>();
        let count = self.listener_count::<T>();

        match metrics.get_mut(&type_id) {
            Some(meta) => {
                meta.update_listener_count(count);
            }
            None => {
                let mut meta = EventMetadata::new::<T>();
                meta.update_listener_count(count);
                metrics.insert(type_id, meta);
            }
        }
    }

    fn check_middleware(&self, event: &dyn Event) -> bool {
        let middleware = self.middleware.read().unwrap();
        middleware.process(event)
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for EventDispatcher {}
unsafe impl Sync for EventDispatcher {}
