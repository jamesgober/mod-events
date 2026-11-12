//! Async event support (requires "async" feature)

use crate::{Event, ListenerError, Priority};
use std::any::Any;
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
/// **Deprecated since 1.2.0.** No [`EventDispatcher`](crate::EventDispatcher)
/// method accepts this trait, so an implementation cannot be registered
/// directly and its `priority()` is never read by the dispatcher. Use a
/// closure with `subscribe_async` or `subscribe_async_with_priority`
/// instead. The trait stays in 1.x for compatibility and is planned for
/// removal in 2.0.
///
/// # Example
///
/// ```rust
/// # #![allow(deprecated)]
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
#[deprecated(
    since = "1.2.0",
    note = "no dispatcher method accepts this trait; register a closure with \
            `subscribe_async` or `subscribe_async_with_priority` instead"
)]
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

/// Type-erased async listener body. Receives the event as
/// `&(dyn Any + Send + Sync)` built by the dispatcher from the concrete
/// `&T`; see `ListenerHandler` in `listener.rs`.
///
/// The `Send + Sync` bound on the argument matters: the dispatcher holds
/// this reference across `.await` points, and a plain `&dyn Any` is not
/// `Send`, which made the `dispatch_async` future `!Send` (it could not
/// run inside `tokio::spawn`). Every `Event` is `Send + Sync`, so the
/// bound costs nothing.
type AsyncEventHandler =
    dyn for<'a> Fn(&'a (dyn Any + Send + Sync)) -> AsyncEventResult<'a> + Send + Sync;

/// A registered async listener plus its ordering metadata. `Clone` is a
/// reference-count increment; see `ListenerWrapper`.
#[derive(Clone)]
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
            handler: Arc::new(move |event: &(dyn Any + Send + Sync)| {
                if let Some(concrete_event) = event.downcast_ref::<T>() {
                    Box::pin(listener(concrete_event))
                } else {
                    // Unreachable: the dispatcher's `TypeId`-keyed registry
                    // only routes matching events here. See the matching
                    // comment in `ListenerWrapper::new`.
                    debug_assert!(
                        false,
                        "AsyncListenerWrapper received event of wrong type: dispatcher routing bug"
                    );
                    Box::pin(async { Ok(()) })
                }
            }),
            priority,
            id,
        }
    }
}
