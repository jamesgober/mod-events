//! Event listener traits and implementations

use crate::{Event, ListenerError, Priority};

/// Trait for synchronous event listeners
///
/// Implement this trait to create reusable event listeners.
/// For simple cases, you can use closures with the dispatcher's
/// `subscribe` or `on` methods instead.
///
/// # Example
///
/// ```rust
/// use mod_events::{Event, EventListener, ListenerError, Priority};
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
/// ```
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

/// Internal listener wrapper for type erasure
type ListenerHandler = dyn Fn(&dyn Event) -> Result<(), ListenerError> + Send + Sync;

pub(crate) struct ListenerWrapper {
    pub(crate) handler: Box<ListenerHandler>,
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
            handler: Box::new(move |event: &dyn Event| {
                if let Some(concrete_event) = event.as_any().downcast_ref::<T>() {
                    listener(concrete_event)
                } else {
                    // The dispatcher's `TypeId`-keyed listener registry
                    // makes this branch unreachable: a listener
                    // registered for `T` only ever receives events of
                    // type `T`. The branch exists to satisfy the
                    // closure's return type; if it ever fires the
                    // dispatcher routing has a bug. Promotes to a
                    // panic in debug builds; silently returns `Ok`
                    // in release to preserve the no-panic contract.
                    debug_assert!(
                        false,
                        "ListenerWrapper received event of wrong type — dispatcher routing bug"
                    );
                    Ok(())
                }
            }),
            priority,
            id,
        }
    }
}
