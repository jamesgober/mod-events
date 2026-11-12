//! Event listener traits and implementations

use crate::{Event, ListenerError, Priority};
use std::any::Any;
use std::sync::Arc;

/// Trait for synchronous event listeners.
///
/// **Deprecated since 1.2.0.** No [`EventDispatcher`](crate::EventDispatcher)
/// method accepts this trait, so an implementation cannot be registered
/// directly and its `priority()` is never read by the dispatcher. Wrap
/// the listener in a closure and pass the priority yourself, as shown
/// below. The trait stays in 1.x for compatibility and is planned for
/// removal in 2.0.
///
/// # Example
///
/// ```rust
/// # #![allow(deprecated)]
/// use mod_events::{Event, EventDispatcher, EventListener, ListenerError, Priority};
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
/// struct EmailNotifier;
///
/// impl EventListener<UserRegistered> for EmailNotifier {
///     fn handle(&self, event: &UserRegistered) -> Result<(), ListenerError> {
///         // Send email logic here
///         println!("Sending email to {}", event.email);
///         Ok(())
///     }
///
///     fn priority(&self) -> Priority {
///         Priority::High
///     }
/// }
///
/// // Register it through a closure.
/// let dispatcher = EventDispatcher::new();
/// let notifier = EmailNotifier;
/// let priority = notifier.priority();
/// dispatcher.subscribe_with_priority(move |e: &UserRegistered| notifier.handle(e), priority);
/// ```
#[deprecated(
    since = "1.2.0",
    note = "no dispatcher method accepts this trait; register a closure instead, \
            e.g. `dispatcher.subscribe_with_priority(move |e| listener.handle(e), listener.priority())`"
)]
pub trait EventListener<T: Event>: Send + Sync {
    /// Handle the event.
    ///
    /// This method is called when the event is dispatched.
    /// Return `Ok(())` on success, or wrap any error in [`ListenerError`]
    /// on failure. `&str` and `String` convert directly via `Into`.
    fn handle(&self, event: &T) -> Result<(), ListenerError>;

    /// Get the priority of this listener
    ///
    /// Higher priority listeners are executed first.
    /// Default is `Priority::Normal`.
    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

/// Type-erased listener body stored in the registry.
///
/// The argument is the dispatched event as `&dyn Any`, produced by the
/// dispatcher from the concrete `&T` it was handed. Routing therefore
/// does not depend on the user's [`Event::as_any`] implementation.
type ListenerHandler = dyn Fn(&dyn Any) -> Result<(), ListenerError> + Send + Sync;

/// A registered listener plus its ordering metadata.
///
/// `Clone` is cheap (one reference-count increment): the registry keeps
/// each event type's listeners in a copy-on-write `Arc<Vec<_>>`, and a
/// subscribe or unsubscribe that races an in-flight dispatch clones the
/// vector instead of waiting for the dispatch to finish.
#[derive(Clone)]
pub(crate) struct ListenerWrapper {
    pub(crate) handler: Arc<ListenerHandler>,
    pub(crate) priority: Priority,
    pub(crate) id: usize,
}

impl std::fmt::Debug for ListenerWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenerWrapper")
            .field("priority", &self.priority)
            .field("id", &self.id)
            .field("handler", &"<function>")
            .finish()
    }
}

impl ListenerWrapper {
    pub(crate) fn new<T, F>(listener: F, priority: Priority, id: usize) -> Self
    where
        T: Event + 'static,
        F: Fn(&T) -> Result<(), ListenerError> + Send + Sync + 'static,
    {
        Self {
            handler: Arc::new(move |event: &dyn Any| {
                if let Some(concrete_event) = event.downcast_ref::<T>() {
                    listener(concrete_event)
                } else {
                    // The registry is keyed by `TypeId::of::<T>()` and the
                    // dispatcher builds the `&dyn Any` from a concrete
                    // `&T`, so this branch is unreachable. It exists to
                    // satisfy the closure's return type; if it ever fires
                    // the dispatcher routing has a bug. Promotes to a
                    // panic in debug builds; returns `Ok` in release to
                    // preserve the no-panic contract.
                    debug_assert!(
                        false,
                        "ListenerWrapper received event of wrong type: dispatcher routing bug"
                    );
                    Ok(())
                }
            }),
            priority,
            id,
        }
    }
}
