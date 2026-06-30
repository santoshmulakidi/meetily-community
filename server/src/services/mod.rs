//! Service layer module
//!
//! Contains business logic and service implementations.

pub mod recording;
pub mod transcription;
pub mod diarization;
pub mod summary;
pub mod search;

// Re-export service traits for easy access
pub use recording::RecordingService;
pub use transcription::TranscriptionService;
pub use diarization::DiarizationService;
pub use summary::SummaryService;
pub use search::SearchService;
