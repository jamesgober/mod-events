//! # Mod Events
//!
//! A high-performance, zero-overhead event dispatcher library for Rust.
//!
//! ## Features
//!
//! - **Zero-cost abstractions**: No runtime overhead for event dispatch
//! - **Type-safe**: Compile-time guarantees for event handling
//! - **Thread-safe**: Built for concurrent applications
//! - **Async support**: Full async/await compatibility (with "async" feature)
//! - **Flexible**: Support for sync, async, and priority-based listeners
//! - **Easy to use**: Simple API and intuitive methods
//!
//! ## Quick Start
//!
//! ```rust
//! use mod_events::prelude::*;
//!
//! // Define your event
//! #[derive(Debug, Clone)]
//! struct UserRegistered {
//!     user_id: u64,
//!     email: String,
//! }
//!
//! impl Event for UserRegistered {
//!     fn as_any(&self) -> &dyn std::any::Any {
//!         self
//!     }
//! }
//!
//! // Create dispatcher and subscribe
//! let dispatcher = EventDispatcher::new();
//! dispatcher.on(|event: &UserRegistered| {
//!     println!("Welcome {}!", event.email);
//! });
//!
//! // Dispatch events
//! dispatcher.emit(UserRegistered {
//!     user_id: 123,
//!     email: "alice@example.com".to_string(),
//! });
//! ```
mod core;
mod dispatcher;
mod listener;
mod metrics;
mod middleware;
mod priority;
mod result;

#[cfg(feature = "async")]
mod async_support;

pub use core::*;
pub use dispatcher::*;
pub use listener::*;
pub use metrics::*;
pub use middleware::*;
pub use priority::*;
pub use result::*;

#[cfg(feature = "async")]
pub use async_support::*;

/// Convenience re-exports
pub mod prelude {
    pub use crate::{Event, EventDispatcher, Priority};

    #[cfg(feature = "async")]
    pub use crate::AsyncEventListener;
}
