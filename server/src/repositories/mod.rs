//! Repository layer module
//!
//! Database access abstractions.

pub mod meeting;
pub mod embedding;

pub use meeting::MeetingRepository;
pub use embedding::EmbeddingRepository;