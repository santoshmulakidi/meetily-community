//! Recording service tests

use super::*;
use crate::config::StorageConfig;
use crate::services::recording::{RecordingService, RecordingConfig, AudioFormat};
use bytes::Bytes;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    async fn test_pool() -> Pool<Postgres> {
        let url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://test:test@localhost:5432/test".to_string());
        
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    /// Helper to create test service
    async fn test_service() -> RecordingServiceImpl {
        let pool = test_pool().await;
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        
        let config = StorageConfig {
            recordings_path: "/tmp/meetily_test_recordings".to_string(),
            max_file_size_mb: 10,
            retention_days: 1,
        };
        
        RecordingServiceImpl::new(config, pool)
    }

    #[tokio::test]
    async fn test_start_recording() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        let session = service.start_recording(meeting_id, user_id, None).await;
        
        assert!(session.is_ok(), "Failed to start recording: {:?}", session.err());
        
        let session = session.unwrap();
        assert_eq!(session.meeting_id, meeting_id);
        assert!(!session.paused);
        assert_eq!(session.chunks_written, 0);
        
        // Cleanup
        let _ = service.stop_recording(session.id).await;
    }

    #[tokio::test]
    async fn test_pause_resume_recording() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // Start recording
        let session = service.start_recording(meeting_id, user_id, None).await.unwrap();
        
        // Pause
        service.pause_recording(session.id).await.unwrap();
        
        // Verify paused
        let status = service.get_session_status(session.id).await.unwrap();
        assert!(status.paused, "Recording should be paused");
        
        // Resume
        service.resume_recording(session.id).await.unwrap();
        
        // Verify resumed
        let status = service.get_session_status(session.id).await.unwrap();
        assert!(!status.paused, "Recording should be resumed");
        
        // Cleanup
        let _ = service.stop_recording(session.id).await;
    }

    #[tokio::test]
    async fn test_write_chunk() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // Start recording
        let session = service.start_recording(meeting_id, user_id, None).await.unwrap();
        
        // Write some dummy audio data (silence)
        let audio_data = Bytes::from(vec![0u8; 4800]); // 100ms at 48kHz, 16-bit, mono
        
        let bytes_written = service.write_chunk(session.id, audio_data, Utc::now()).await;
        
        assert!(bytes_written.is_ok(), "Failed to write chunk: {:?}", bytes_written.err());
        assert!(bytes_written.unwrap() > 0, "Should have written some bytes");
        
        // Cleanup
        let _ = service.stop_recording(session.id).await;
    }

    #[tokio::test]
    async fn test_crash_recovery() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // Start recording
        let session = service.start_recording(meeting_id, user_id, None).await.unwrap();
        let session_id = session.id;
        
        // Write a few chunks
        for i in 0..5 {
            let audio_data = Bytes::from(vec![i; 4800]);
            service.write_chunk(session_id, audio_data, Utc::now()).await.unwrap();
            sleep(Duration::from_millis(10)).await;
        }
        
        // Simulate crash by removing from memory (but checkpoint should exist in DB)
        {
            let mut sessions = service.sessions.write().await;
            sessions.retain(|s| s.session.id != session_id);
        }
        
        // Verify session is gone from memory
        assert!(service.get_session_status(session_id).await.is_err(), 
                "Session should not be in memory");
        
        // Recover from checkpoint
        let recovered = service.recover_session(session_id).await;
        
        assert!(recovered.is_ok(), "Failed to recover session: {:?}", recovered.err());
        
        let recovered = recovered.unwrap();
        assert_eq!(recovered.id, session_id);
        assert!(recovered.chunks_written >= 5, "Should have recovered chunk count");
        
        // Cleanup
        let _ = service.stop_recording(session_id).await;
    }

    #[tokio::test]
    async fn test_stop_recording_creates_metadata() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // Start recording
        let session = service.start_recording(meeting_id, user_id, None).await.unwrap();
        
        // Write some data
        for _ in 0..3 {
            let audio_data = Bytes::from(vec![0u8; 4800]);
            service.write_chunk(session.id, audio_data, Utc::now()).await.unwrap();
            sleep(Duration::from_millis(50)).await;
        }
        
        // Stop recording
        let metadata = service.stop_recording(session.id).await.unwrap();
        
        assert_eq!(metadata.meeting_id, meeting_id);
        assert_eq!(metadata.status, RecordingStatus::Completed);
        assert!(metadata.duration_secs > 0, "Should have positive duration");
        assert!(metadata.file_size_bytes > 0, "Should have positive file size");
        assert!(!metadata.file_path.is_empty(), "Should have file path");
        
        // Verify file exists
        let file_exists = tokio::fs::metadata(&metadata.file_path).await.is_ok();
        assert!(file_exists, "Recording file should exist: {}", metadata.file_path);
    }

    #[tokio::test]
    async fn test_file_rotation() {
        let service = test_service().await;
        
        let meeting_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // Start recording with very small file size limit
        let config = RecordingConfig {
            sample_rate: 48000,
            channels: 2,
            bit_depth: 16,
            format: AudioFormat::Wav,
            chunk_duration_secs: 1,
            max_file_size_mb: Some(1), // 1MB limit for testing
            storage_path: service.config.recordings_path.clone(),
        };
        
        let session = service.start_recording(meeting_id, user_id, Some(config)).await.unwrap();
        
        // Write enough data to trigger rotation (1MB = ~1M bytes, we write ~5KB per chunk)
        for i in 0..250 {
            let audio_data = Bytes::from(vec![i as u8; 5000]);
            service.write_chunk(session.id, audio_data, Utc::now()).await.unwrap();
        }
        
        // Stop and verify
        let metadata = service.stop_recording(session.id).await.unwrap();
        
        // Should have created multiple chunk files
        let chunk_count = glob::glob(&format!("{}*", metadata.file_path.replace(".wav", "_chunk_")))
            .unwrap()
            .count();
        
        assert!(chunk_count > 1, "Should have rotated files (found {} chunks)", chunk_count);
    }
}

use super::*;