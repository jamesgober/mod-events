//! Error types exposed by the dispatcher.
//!
//! The crate distinguishes two failure surfaces:
//!
//! - [`ListenerError`] wraps the error value returned by a user-supplied
//!   listener. Listeners can construct one from any `Error + Send + Sync +
//!   'static`, from a `&str`, from a `String`, or from a pre-existing
//!   `Box<dyn Error + Send + Sync>`. The dispatcher itself never inspects
//!   the inner error — it only stores and exposes it through
//!   [`crate::DispatchResult`].
//!
//! There is intentionally no top-level `DispatchError` enum yet: every
//! dispatcher method is currently infallible because the underlying lock
//! primitive (`parking_lot::RwLock`) does not poison. A new enum will be
//! added the first time a dispatcher operation grows a real failure mode.

use std::error::Error;
use std::fmt;

/// Opaque error returned by an event listener.
///
/// `ListenerError` is the typed wrapper the dispatcher stores in
/// [`crate::DispatchResult`] for every failing listener. It hides the
/// concrete error type a listener returned, while still letting callers
/// inspect the chain via [`std::error::Error::source`].
///
/// # Constructing
///
/// ```rust
/// use mod_events::ListenerError;
/// use std::io;
///
/// // From any Error + Send + Sync + 'static
/// let from_err = ListenerError::new(io::Error::new(io::ErrorKind::Other, "bad"));
///
/// // From a string literal or owned String, via Into
/// let from_str: ListenerError = "validation failed".into();
/// let from_string: ListenerError = format!("retry budget exhausted").into();
/// ```
#[derive(Debug)]
pub struct ListenerError(Box<dyn Error + Send + Sync + 'static>);

impl ListenerError {
    /// Wrap any error value that implements `Error + Send + Sync + 'static`.
    pub fn new<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self(Box::new(error))
    }

    /// Wrap a textual message as a listener error.
    pub fn message<S: Into<String>>(msg: S) -> Self {
        Self(msg.into().into())
    }

    /// Borrow the underlying error trait object.
    #[must_use]
    pub fn inner(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.0.as_ref()
    }

    /// Consume the wrapper and return the inner boxed error.
    #[must_use]
    pub fn into_inner(self) -> Box<dyn Error + Send + Sync + 'static> {
        self.0
    }
}

impl fmt::Display for ListenerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for ListenerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

impl From<Box<dyn Error + Send + Sync + 'static>> for ListenerError {
    fn from(boxed: Box<dyn Error + Send + Sync + 'static>) -> Self {
        Self(boxed)
    }
}

impl From<&str> for ListenerError {
    fn from(s: &str) -> Self {
        Self::message(s)
    }
}

impl From<String> for ListenerError {
    fn from(s: String) -> Self {
        Self::message(s)
    }
}

/// Convert a panic payload (the value carried out of a unwinding
/// listener) into a [`ListenerError`].
///
/// `std::panic::catch_unwind` returns the panic payload as
/// `Box<dyn Any + Send>`. The dispatcher uses this helper to turn that
/// payload into the same `ListenerError` shape a well-behaved listener
/// would have returned, so `DispatchResult::errors()` is the single
/// place the caller has to look for failures — panics included.
///
/// Most panics carry a `&'static str` (from `panic!("…")`) or a
/// `String` (from `panic!("{}", x)`); both are extracted directly.
/// Anything else degrades to a generic `"listener panicked"` message.
pub(crate) fn panic_payload_to_listener_error(
    payload: Box<dyn std::any::Any + Send>,
) -> ListenerError {
    let detail = if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        String::from("<non-string panic payload>")
    };
    ListenerError::message(format!("listener panicked: {detail}"))
}
