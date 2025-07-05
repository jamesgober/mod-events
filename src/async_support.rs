//! Async event support (requires "async" feature)

use crate::{Event, Priority};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Trait for asynchronous event listeners
///
/// This trait is only available when the "async" feature is enabled.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "async")]
/// # {
/// use mod_events::{AsyncEventListener, Priority, Event};
/// use std::future::Future;
/// use std::pin::Pin;
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
///     fn handle<'a>(&'a self, event: &'a UserRegistered) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
///         Box::pin(async move {
///             // Async email sending logic
///             println!("Async email sent to {}", event.email);
///             Ok(())
///         })
///     }
/// }
/// # }
/// ```
pub type AsyncEventResult<'a> =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>;

pub trait AsyncEventListener<T: Event>: Send + Sync {
    /// Handle the event asynchronously
    fn handle<'a>(&'a self, event: &'a T) -> AsyncEventResult<'a>;

    /// Get the priority of this listener
    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

/// Internal async listener wrapper
/// Type alias for the async event handler function
type AsyncEventHandler = dyn for<'a> Fn(
        &'a dyn Event,
    ) -> Pin<
        Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>,
    > + Send
    + Sync;

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
        Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        Self {
            handler: Arc::new(move |event: &dyn Event| {
                if let Some(concrete_event) = event.as_any().downcast_ref::<T>() {
                    Box::pin(listener(concrete_event))
                } else {
                    Box::pin(async { Ok(()) })
                }
            }),
            priority,
            id,
        }
    }
}
