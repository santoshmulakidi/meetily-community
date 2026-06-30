-- Migration: Create recording tables
-- Version: 001

-- Recording sessions (for crash recovery)
CREATE TABLE IF NOT EXISTS recording_sessions (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    config_json JSONB NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    paused BOOLEAN NOT NULL DEFAULT FALSE,
    chunks_written BIGINT NOT NULL DEFAULT 0,
    current_file_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Completed recordings
CREATE TABLE IF NOT EXISTS recordings (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    user_id UUID NOT NULL,
    device_name TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    duration_secs BIGINT NOT NULL,
    file_size_bytes BIGINT NOT NULL,
    file_path TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_recordings_meeting_id ON recordings(meeting_id);
CREATE INDEX IF NOT EXISTS idx_sessions_meeting_id ON recording_sessions(meeting_id);
CREATE INDEX IF NOT EXISTS idx_recordings_created_at ON recordings(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_recordings_status ON recordings(status);

-- Comments
COMMENT ON TABLE recording_sessions IS 'Active/paused recording sessions for crash recovery';
COMMENT ON TABLE recordings IS 'Completed recording metadata';
COMMENT ON COLUMN recordings.metadata_json IS 'Additional metadata (device info, settings, etc.)';