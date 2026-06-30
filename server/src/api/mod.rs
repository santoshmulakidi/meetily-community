//! API layer module
//!
//! HTTP routes, handlers, and middleware.

mod routes;
mod handlers;
mod state;

pub use routes::create_router;
pub use state::SharedState;