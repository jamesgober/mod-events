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
    /// Returns the event as [`Any`] for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns the unique [`TypeId`] identifier for this event type.
    ///
    /// Equivalent to `<Self as Any>::type_id(self)` since `Event`
    /// requires the [`Any`] supertrait. Both methods are available and
    /// return identical values; this one is provided for ergonomics
    /// when working with `&dyn Event` trait objects, where the
    /// supertrait method requires a `&dyn Any` cast first.
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    /// Returns the event name for debugging / logging.
    ///
    /// Backed by [`std::any::type_name`]. The exact format is
    /// **not stable** across compiler versions — the Rust standard
    /// library reserves the right to change `type_name` output
    /// between releases. Treat the result as opaque human-readable
    /// text. Do not parse it, persist it, or use it as a stable
    /// cross-process identifier; use [`Event::type_id`] for that
    /// instead.
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
