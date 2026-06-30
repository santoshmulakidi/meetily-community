-- Migration: Create conversations and messages tables
-- Version: 006

-- Conversations (chat sessions)
CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY,
    meeting_ids_json JSONB NOT NULL,  -- Array of meeting IDs this conversation is about
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Conversation messages (chat history)
CREATE TABLE IF NOT EXISTS conversation_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,  -- 'user' or 'assistant'
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_conversation_id ON conversation_messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_created_at ON conversation_messages(created_at ASC);

-- Comments
COMMENT ON TABLE conversations IS 'Chat sessions with users (ChatGPT-style interface)';
COMMENT ON TABLE conversation_messages IS 'Individual messages in chat conversations';
COMMENT ON COLUMN conversations.meeting_ids_json IS 'Array of meeting IDs this conversation context is scoped to: ["uuid1", "uuid2"]';
COMMENT ON COLUMN conversation_messages.role IS 'Message role: "user" or "assistant"';

-- Helper view: Recent conversations with message count
CREATE OR REPLACE VIEW recent_conversations AS
SELECT 
    c.id,
    c.meeting_ids_json,
    c.created_at,
    c.updated_at,
    COUNT(m.id) as message_count,
    array_agg(m.role ORDER BY m.created_at) as message_roles
FROM conversations c
LEFT JOIN conversation_messages m ON c.id = m.conversation_id
GROUP BY c.id, c.meeting_ids_json, c.created_at, c.updated_at
ORDER BY c.updated_at DESC;

-- Helper function: Get conversation summary (first user message + last assistant message)
CREATE OR REPLACE FUNCTION get_conversation_summary(conv_id UUID)
RETURNS TABLE (
    first_user_message TEXT,
    last_assistant_message TEXT,
    total_messages BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        (SELECT content FROM conversation_messages WHERE conversation_id = conv_id AND role = 'user' ORDER BY created_at ASC LIMIT 1) as first_user_message,
        (SELECT content FROM conversation_messages WHERE conversation_id = conv_id AND role = 'assistant' ORDER BY created_at DESC LIMIT 1) as last_assistant_message,
        (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id = conv_id) as total_messages;
END;
$$ LANGUAGE plpgsql STABLE;