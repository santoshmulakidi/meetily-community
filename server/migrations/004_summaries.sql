-- Migration: Create summaries table
-- Version: 004

-- AI-generated summaries
CREATE TABLE IF NOT EXISTS summaries (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    summary_type TEXT NOT NULL,
    content TEXT NOT NULL,
    model_used TEXT NOT NULL,
    provider TEXT NOT NULL,
    metadata_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_summaries_meeting_id ON summaries(meeting_id);
CREATE INDEX IF NOT EXISTS idx_summaries_type ON summaries(summary_type);
CREATE INDEX IF NOT EXISTS idx_summaries_created_at ON summaries(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_summaries_meeting_type ON summaries(meeting_id, summary_type);

-- Comments
COMMENT ON TABLE summaries IS 'AI-generated meeting summaries (executive, technical, action items, etc.)';
COMMENT ON COLUMN summaries.summary_type IS 'Type of summary: executive, technical, action_items, decisions, risks, follow_up, custom';
COMMENT ON COLUMN summaries.metadata_json IS 'Summary metadata: {"prompt_tokens": N, "completion_tokens": N, "total_tokens": N, "cost_usd": X.XX}';
COMMENT ON COLUMN summaries.model_used IS 'LLM model used (e.g., "llama3.1:8b", "claude-3.5-sonnet")';
COMMENT ON COLUMN summaries.provider IS 'Provider used (e.g., "ollama", "openrouter", "nvidia")';