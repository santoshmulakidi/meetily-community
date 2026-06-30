//! Service layer module
//!
//! Contains business logic and service implementations.

pub mod recording;
pub mod transcription;
pub mod diarization;
pub mod summary;
pub mod embedding;
pub mod search;
pub mod chat;

// Re-export service traits for easy access
pub use recording::RecordingService;
pub use transcription::TranscriptionService;
pub use diarization::DiarizationService;
pub use summary::SummaryService;
pub use embedding::EmbeddingService;
pub use search::SearchService;
pub use chat::ChatService;