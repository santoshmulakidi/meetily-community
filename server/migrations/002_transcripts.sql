-- Migration: Create transcription tables
-- Version: 002

-- Transcripts
CREATE TABLE IF NOT EXISTS transcripts (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    recording_id UUID,
    language TEXT NOT NULL,
    duration_secs REAL NOT NULL,
    word_count BIGINT NOT NULL,
    status TEXT NOT NULL,
    metadata_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Transcript segments (individual speech segments with timing)
CREATE TABLE IF NOT EXISTS transcript_segments (
    id UUID PRIMARY KEY,
    transcript_id UUID NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
    start_time_secs REAL NOT NULL,
    end_time_secs REAL NOT NULL,
    text TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    speaker_id TEXT,
    language TEXT NOT NULL,
    words_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_transcripts_meeting_id ON transcripts(meeting_id);
CREATE INDEX IF NOT EXISTS idx_transcripts_recording_id ON transcripts(recording_id);
CREATE INDEX IF NOT EXISTS idx_transcripts_created_at ON transcripts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_segments_transcript_id ON transcript_segments(transcript_id);
CREATE INDEX IF NOT EXISTS idx_segments_speaker_id ON transcript_segments(speaker_id) WHERE speaker_id IS NOT NULL;

-- Full-text search on transcript text
CREATE INDEX IF NOT EXISTS idx_segments_text_search ON transcript_segments USING gin(to_tsvector('english', text));

-- Comments
COMMENT ON TABLE transcripts IS 'Complete meeting transcripts with metadata';
COMMENT ON TABLE transcript_segments IS 'Individual speech segments with word-level timestamps';
COMMENT ON COLUMN transcript_segments.words_json IS 'Word-level timestamps: [{"word": "hello", "start": 0.0, "end": 0.5, "confidence": 0.95}]';
COMMENT ON COLUMN transcripts.metadata_json IS 'Transcription metadata: {"model_name": "...", "provider": "...", "processing_time_secs": ..., "gpu_accelerated": ...}';