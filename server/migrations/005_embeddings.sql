-- Migration: Create embeddings table with pgvector
-- Version: 005

-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Embeddings table (vector storage)
CREATE TABLE IF NOT EXISTS embeddings (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    transcript_id UUID,
    chunk_text TEXT NOT NULL,
    vector vector(3072) NOT NULL,  -- Max dimension (OpenAI large)
    dimension BIGINT NOT NULL,
    model TEXT NOT NULL,
    provider TEXT NOT NULL,
    metadata_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_embeddings_meeting_id ON embeddings(meeting_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_transcript_id ON embeddings(transcript_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_created_at ON embeddings(created_at DESC);

-- Vector similarity index (IVFFlat for fast approximate search)
-- Lists parameter: typically 4 * sqrt(N) where N = number of rows
-- For 100K embeddings: 4 * sqrt(100000) ≈ 1260, round to 1000
CREATE INDEX IF NOT EXISTS idx_embeddings_vector_cosine 
    ON embeddings USING ivfflat (vector vector_cosine_ops) 
    WITH (lists = 1000);

-- Alternative index for inner product (dot product similarity)
-- CREATE INDEX IF NOT EXISTS idx_embeddings_vector_ip 
--     ON embeddings USING ivfflat (vector vector_ip_ops) 
--     WITH (lists = 1000);

-- Comments
COMMENT ON TABLE embeddings IS 'Vector embeddings of meeting transcript chunks for semantic search';
COMMENT ON COLUMN embeddings.vector IS 'Embedding vector (dimension varies by model: 384-3072)';
COMMENT ON COLUMN embeddings.dimension IS 'Vector dimension (384, 768, 1024, 1536, 3072)';
COMMENT ON COLUMN embeddings.metadata_json IS 'Metadata: {"chunk_index": N, "start_time_secs": X, "end_time_secs": Y, "speaker_id": "...", "embedding_type": "..."}';
COMMENT ON COLUMN embeddings.meeting_id IS 'Meeting this embedding belongs to (for scoped search)';
COMMENT ON COLUMN embeddings.transcript_id IS 'Source transcript (nullable for summary/action item embeddings)';

-- Helper function: Create embedding for query (same model as stored embeddings)
-- Usage: SELECT cosine_similarity(query_vector, vector) as similarity FROM embeddings ORDER BY similarity DESC LIMIT 10;
CREATE OR REPLACE FUNCTION cosine_similarity(vec1 vector, vec2 vector)
RETURNS float AS $$
BEGIN
    RETURN 1 - (vec1 <=> vec2);  -- <=> is cosine distance, so 1 - distance = similarity
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Helper view: Recent embeddings by meeting
CREATE OR REPLACE VIEW recent_embeddings_by_meeting AS
SELECT 
    meeting_id,
    COUNT(*) as embedding_count,
    MAX(created_at) as last_updated,
    array_agg(DISTINCT model) as models_used,
    array_agg(DISTINCT provider) as providers_used
FROM embeddings
GROUP BY meeting_id
ORDER BY last_updated DESC;

-- Grant permissions (adjust as needed)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON embeddings TO meetily_app;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO meetily_app;