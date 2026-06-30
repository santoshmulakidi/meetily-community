-- Migration: Create diarization tables
-- Version: 003

-- Diarization results (per meeting)
CREATE TABLE IF NOT EXISTS diarizations (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    num_speakers BIGINT NOT NULL,
    duration_secs REAL NOT NULL,
    metadata_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Diarization segments (speaker timestamps)
CREATE TABLE IF NOT EXISTS diarization_segments (
    id UUID PRIMARY KEY,
    diarization_id UUID NOT NULL REFERENCES diarizations(id) ON DELETE CASCADE,
    start_time_secs REAL NOT NULL,
    end_time_secs REAL NOT NULL,
    speaker_id TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Speaker aliases (manual renaming)
CREATE TABLE IF NOT EXISTS speaker_aliases (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    original_speaker_id TEXT NOT NULL,
    custom_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_diarizations_meeting_id ON diarizations(meeting_id);
CREATE INDEX IF NOT EXISTS idx_diarization_segments_diarization_id ON diarization_segments(diarization_id);
CREATE INDEX IF NOT EXISTS idx_speaker_aliases_meeting_id ON speaker_aliases(meeting_id);
CREATE INDEX IF NOT EXISTS idx_speaker_aliases_lookup ON speaker_aliases(meeting_id, original_speaker_id);

-- Comments
COMMENT ON TABLE diarizations IS 'Speaker diarization results for meetings';
COMMENT ON TABLE diarization_segments IS 'Individual speaker segments with timestamps';
COMMENT ON TABLE speaker_aliases IS 'Manual speaker name mappings (SPEAKER_00 -> "John Doe")';
COMMENT ON COLUMN diarizations.metadata_json IS 'Diarization metadata: {"provider": "...", "model": "...", "processing_time_secs": ...}';