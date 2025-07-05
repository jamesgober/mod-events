//! Event listener traits and implementations

use crate::{Event, Priority};

/// Trait for synchronous event listeners
///
/// Implement this trait to create reusable event listeners.
/// For simple cases, you can use closures with the dispatcher's
/// `subscribe` or `on` methods instead.
///
/// # Example
///
/// ```rust
/// use mod_events::{Event, EventListener, Priority};
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
///     fn handle(&self, event: &UserRegistered) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    /// Handle the event
    ///
    /// This method is called when the event is dispatched.
    /// Return `Ok(())` on success or an error on failure.
    fn handle(&self, event: &T) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get the priority of this listener
    ///
    /// Higher priority listeners are executed first.
    /// Default is `Priority::Normal`.
    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

/// Internal listener wrapper for type erasure
type ListenerHandler =
    dyn Fn(&dyn Event) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync;

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
        F: Fn(&T) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
    {
        Self {
            handler: Box::new(move |event: &dyn Event| {
                if let Some(concrete_event) = event.as_any().downcast_ref::<T>() {
                    listener(concrete_event)
                } else {
                    Ok(())
                }
            }),
            priority,
            id,
        }
    }
}
