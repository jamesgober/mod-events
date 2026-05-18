//! Async event support (requires "async" feature)

use crate::{Event, ListenerError, Priority};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Pinned future returned by an async listener.
pub type AsyncEventResult<'a> =
    Pin<Box<dyn Future<Output = Result<(), ListenerError>> + Send + 'a>>;

/// Trait for asynchronous event listeners.
///
/// This trait is only available when the `async` feature is enabled.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "async")]
/// # {
/// use mod_events::{AsyncEventListener, AsyncEventResult, Event, ListenerError, Priority};
///
/// #[derive(Debug, Clone)]
/// struct UserRegistered {
///     user_id: u64,
///     email: String,
/// }
///
/// impl Event for UserRegistered {
///     fn as_any(&self) -> &dyn std::any::Any {
///         self
///     }
/// }
///
/// struct AsyncEmailNotifier;
///
/// impl AsyncEventListener<UserRegistered> for AsyncEmailNotifier {
///     fn handle<'a>(&'a self, event: &'a UserRegistered) -> AsyncEventResult<'a> {
///         Box::pin(async move {
///             println!("Async email sent to {}", event.email);
///             Ok(())
///         })
///     }
/// }
/// # }
/// ```
pub trait AsyncEventListener<T: Event>: Send + Sync {
    /// Handle the event asynchronously.
    fn handle<'a>(&'a self, event: &'a T) -> AsyncEventResult<'a>;

    /// Get the priority of this listener.
    ///
    /// Higher priority listeners are executed first. Default is
    /// [`Priority::Normal`].
    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

/// Internal async listener wrapper.
type AsyncEventHandler = dyn for<'a> Fn(&'a dyn Event) -> AsyncEventResult<'a> + Send + Sync;

pub(crate) struct AsyncListenerWrapper {
    pub(crate) handler: Arc<AsyncEventHandler>,
    pub(crate) priority: Priority,
    pub(crate) id: usize,
}

impl std::fmt::Debug for AsyncListenerWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncListenerWrapper")
            .field("priority", &self.priority)
            .field("id", &self.id)
            .field("handler", &"<async_function>")
            .finish()
    }
}

impl AsyncListenerWrapper {
    pub(crate) fn new<T, F, Fut>(listener: F, priority: Priority, id: usize) -> Self
    where
        T: Event + 'static,
        F: Fn(&T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), ListenerError>> + Send + 'static,
    {
        Self {
            handler: Arc::new(move |event: &dyn Event| {
                if let Some(concrete_event) = event.as_any().downcast_ref::<T>() {
                    Box::pin(listener(concrete_event))
                } else {
                    // Unreachable in practice — the dispatcher's
                    // `TypeId`-keyed registry guarantees only matching
                    // events reach this wrapper. See the matching
                    // comment in `ListenerWrapper::new`.
                    debug_assert!(
                        false,
                        "AsyncListenerWrapper received event of wrong type — dispatcher routing bug"
                    );
                    Box::pin(async { Ok(()) })
                }
            }),
            priority,
            id,
        }
    }
}
