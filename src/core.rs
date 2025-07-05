//! Core event system traits and types

use std::any::{Any, TypeId};
use std::fmt;

/// Core trait that all events must implement
///
/// This trait provides the foundation for type-safe event dispatch.
/// All event types must implement this trait to be used with the dispatcher.
///
/// # Example
///
/// ```rust
/// use mod_events::Event;
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
/// ```
pub trait Event: Any + Send + Sync + fmt::Debug {
    /// Returns the event as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Returns a unique identifier for this event type
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    /// Returns the event name for debugging
    fn event_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Unique identifier for event listeners
///
/// This is returned when subscribing to events and can be used
/// to unsubscribe specific listeners later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId {
    pub(crate) id: usize,
    pub(crate) type_id: TypeId,
}

impl ListenerId {
    pub(crate) fn new(id: usize, type_id: TypeId) -> Self {
        Self { id, type_id }
    }
}
