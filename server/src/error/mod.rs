//! Error handling module
//!
//! Provides unified error types and conversion to HTTP responses.

mod error;
mod handlers;

pub use error::*;
pub use handlers::*;