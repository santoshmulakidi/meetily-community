//! Recording Service Unit Tests

use chrono::Utc;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

use crate::services::recording::{RecordingService, RecordingServiceImpl, RecordingConfig};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_recording_service() -> (RecordingServiceImpl, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config = RecordingConfig {
        storage_path: temp_dir.path().to_path_buf(),
        max_file_size_mb: 10,
        max_duration_minutes: 60,
    };
    
    let service = RecordingServiceImpl::new(config);
    (service, temp_dir)
}

// ============================================================================
// Recording Service Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_recording() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let result = service.start_recording(meeting_id).await;
        
        assert!(result.is_ok(), "Failed to start recording: {:?}", result);
        
        let session = result.unwrap();
        assert_eq!(session.meeting_id, meeting_id);
        assert!(session.is_active);
        assert!(session.started_at <= Utc::now());
    }

    #[tokio::test]
    async fn test_stop_recording() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        
        // Simulate some recording time
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let recording = service.stop_recording(session.id).await.unwrap();
        
        assert_eq!(recording.meeting_id, meeting_id);
        assert!(!recording.is_active);
        assert!(recording.stopped_at.is_some());
        assert!(recording.duration_seconds > 0);
    }

    #[tokio::test]
    async fn test_pause_recording() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        
        let paused_session = service.pause_recording(session.id).await.unwrap();
        
        assert!(!paused_session.is_active);
        assert!(paused_session.paused_at.is_some());
    }

    #[tokio::test]
    async fn test_resume_recording() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        let paused_session = service.pause_recording(session.id).await.unwrap();
        
        let resumed_session = service.resume_recording(paused_session.id).await.unwrap();
        
        assert!(resumed_session.is_active);
        assert!(resumed_session.resumed_at.is_some());
    }

    #[tokio::test]
    async fn test_recording_file_rotation() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        
        // Simulate multiple file rotations
        for i in 0..3 {
            service.rotate_file(session.id).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        
        let recording = service.stop_recording(session.id).await.unwrap();
        
        assert!(recording.file_paths.len() >= 3);
    }

    #[tokio::test]
    async fn test_get_active_sessions() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id1 = Uuid::new_v4();
        let meeting_id2 = Uuid::new_v4();
        
        let _session1 = service.start_recording(meeting_id1).await.unwrap();
        let _session2 = service.start_recording(meeting_id2).await.unwrap();
        
        let active_sessions = service.get_active_sessions().await.unwrap();
        
        assert!(active_sessions.len() >= 2);
    }

    #[tokio::test]
    async fn test_get_recording_by_id() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        let recording = service.stop_recording(session.id).await.unwrap();
        
        let retrieved = service.get_recording(recording.id).await.unwrap();
        
        assert_eq!(retrieved.id, recording.id);
        assert_eq!(retrieved.meeting_id, meeting_id);
    }

    #[tokio::test]
    async fn test_crash_recovery() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        
        // Simulate crash by not stopping properly
        drop(session);
        
        // Recovery should detect incomplete sessions
        let recovered = service.recover_incomplete_sessions().await.unwrap();
        
        assert!(!recovered.is_empty());
    }

    #[tokio::test]
    async fn test_recording_statistics() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let meeting_id = Uuid::new_v4();
        let session = service.start_recording(meeting_id).await.unwrap();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        let recording = service.stop_recording(session.id).await.unwrap();
        
        assert!(recording.duration_seconds > 0);
        assert!(recording.file_size_bytes > 0);
        assert_eq!(recording.format, "wav");
        assert_eq!(recording.sample_rate, 16000);
        assert_eq!(recording.channels, 1);
    }

    #[tokio::test]
    async fn test_concurrent_recordings() {
        let (service, _temp_dir) = create_test_recording_service();
        
        let mut handles = vec![];
        
        // Start 5 concurrent recordings
        for i in 0..5 {
            let service_clone = service.clone();
            let meeting_id = Uuid::new_v4();
            
            let handle = tokio::spawn(async move {
                service_clone.start_recording(meeting_id).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all to start
        let results = futures::future::join_all(handles).await;
        
        // All should succeed
        for result in results {
            assert!(result.unwrap().is_ok());
        }
        
        // Verify all are active
        let active_sessions = service.get_active_sessions().await.unwrap();
        assert!(active_sessions.len() >= 5);
    }
}