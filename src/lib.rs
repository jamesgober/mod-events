//! # Mod Events
//!
//! A high-performance, zero-overhead event dispatcher library for Rust.
//!
//! ## Features
//!
//! - Zero-cost abstractions: no runtime overhead for event dispatch.
//! - Type-safe: compile-time guarantees for event handling.
//! - Thread-safe: built for concurrent applications.
//! - Async support: full `async`/`.await` compatibility (with the `async`
//!   feature).
//! - Flexible: sync, async, and priority-based listeners.
//! - Simple API.
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

#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::unreachable)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

mod core;
mod dispatcher;
mod error;
mod listener;
mod metrics;
mod middleware;
mod priority;
mod result;
mod type_id_map;

#[cfg(feature = "async")]
mod async_support;

pub use core::*;
pub use dispatcher::*;
pub use error::*;
pub use listener::*;
pub use metrics::*;
pub use priority::*;
pub use result::*;

#[cfg(feature = "async")]
pub use async_support::*;

/// Convenience re-exports
pub mod prelude {
    pub use crate::{Event, EventDispatcher, ListenerError, Priority};

    #[cfg(feature = "async")]
    pub use crate::AsyncEventListener;
}
