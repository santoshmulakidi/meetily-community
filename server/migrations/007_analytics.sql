-- Migration: Create analytics views and indexes
-- Version: 007

-- ponytail: earlier migrations reference meeting_id but never create meetings; this minimal table unblocks analytics/auth views.
CREATE TABLE IF NOT EXISTS meetings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID,
    name TEXT NOT NULL DEFAULT 'Untitled Meeting',
    title TEXT,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'created',
    tags TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create analytics-friendly indexes
CREATE INDEX IF NOT EXISTS idx_meetings_created_at ON meetings(created_at DESC);
-- ponytail: DATE(timestamptz) is not immutable, plain created_at index is enough for deploy.
CREATE INDEX IF NOT EXISTS idx_recordings_meeting_id_duration ON recordings(meeting_id, duration_secs);
CREATE INDEX IF NOT EXISTS idx_transcripts_meeting_id ON transcripts(meeting_id);
CREATE INDEX IF NOT EXISTS idx_summaries_meeting_id_type ON summaries(meeting_id, summary_type);
CREATE INDEX IF NOT EXISTS idx_diarization_segments_speaker ON diarization_segments(diarization_id, speaker_id);

-- View: Meeting statistics by date
CREATE OR REPLACE VIEW v_meeting_daily_stats AS
SELECT 
    DATE(m.created_at) as date,
    COUNT(DISTINCT m.id) as meeting_count,
    COALESCE(SUM(r.duration_secs), 0) as total_duration_secs,
    COALESCE(AVG(r.duration_secs), 0) as avg_duration_secs,
    COUNT(DISTINCT t.id) as transcripts_count,
    COUNT(DISTINCT s.id) as summaries_count,
    COUNT(DISTINCT d.id) as diarizations_count
FROM meetings m
LEFT JOIN recordings r ON m.id = r.meeting_id
LEFT JOIN transcripts t ON m.id = t.meeting_id
LEFT JOIN summaries s ON m.id = s.meeting_id
LEFT JOIN diarizations d ON m.id = d.meeting_id
GROUP BY DATE(m.created_at)
ORDER BY date DESC;

-- View: Speaker statistics per meeting
CREATE OR REPLACE VIEW v_speaker_stats_per_meeting AS
SELECT 
    d.meeting_id,
    COALESCE(ds.speaker_id, 'Unknown') as speaker_id,
    COALESCE(sa.custom_name, ds.speaker_id) as speaker_name,
    COUNT(DISTINCT ds.id) as segment_count,
    SUM(GREATEST(ds.end_time_secs - ds.start_time_secs, 0)) as total_talk_time_secs,
    AVG(COALESCE(ds.confidence, 0.0)) as avg_confidence,
    RANK() OVER (PARTITION BY d.meeting_id ORDER BY SUM(GREATEST(ds.end_time_secs - ds.start_time_secs, 0)) DESC) as talk_rank
FROM diarizations d
JOIN diarization_segments ds ON d.id = ds.diarization_id
LEFT JOIN speaker_aliases sa ON ds.speaker_id = sa.original_speaker_id AND sa.meeting_id = d.meeting_id
GROUP BY d.meeting_id, ds.speaker_id, sa.custom_name
ORDER BY d.meeting_id, talk_rank;

-- View: Overall speaker statistics across all meetings
CREATE OR REPLACE VIEW v_speaker_overall_stats AS
SELECT 
    COALESCE(ds.speaker_id, 'Unknown') as speaker_id,
    COUNT(DISTINCT ds.id) as total_segments,
    SUM(GREATEST(ds.end_time_secs - ds.start_time_secs, 0)) as total_talk_time_secs,
    AVG(COALESCE(ds.confidence, 0.0)) as avg_confidence,
    COUNT(DISTINCT d.meeting_id) as meetings_participated,
    COUNT(DISTINCT d.meeting_id) * 1.0 / (SELECT COUNT(*) FROM meetings) as participation_rate
FROM diarization_segments ds
JOIN diarizations d ON ds.diarization_id = d.id
GROUP BY ds.speaker_id
ORDER BY meetings_participated DESC, total_talk_time_secs DESC;

-- View: Summary type distribution
CREATE OR REPLACE VIEW v_summary_type_distribution AS
SELECT 
    summary_type,
    COUNT(*) as count,
    COUNT(DISTINCT meeting_id) as meetings_covered,
    AVG(LENGTH(content)) as avg_content_length
FROM summaries
GROUP BY summary_type
ORDER BY count DESC;

-- View: Meeting completeness (what stages completed)
CREATE OR REPLACE VIEW v_meeting_completeness AS
SELECT 
    m.id as meeting_id,
    m.created_at,
    (r.id IS NOT NULL) as has_recording,
    (t.id IS NOT NULL) as has_transcript,
    (d.id IS NOT NULL) as has_diarization,
    (s.id IS NOT NULL) as has_summary,
    (e.id IS NOT NULL) as has_embeddings,
    -- Calculate completeness score (0-100)
    (
        (CASE WHEN r.id IS NOT NULL THEN 20 ELSE 0 END) +
        (CASE WHEN t.id IS NOT NULL THEN 20 ELSE 0 END) +
        (CASE WHEN d.id IS NOT NULL THEN 20 ELSE 0 END) +
        (CASE WHEN s.id IS NOT NULL THEN 20 ELSE 0 END) +
        (CASE WHEN e.id IS NOT NULL THEN 20 ELSE 0 END)
    ) as completeness_score
FROM meetings m
LEFT JOIN recordings r ON m.id = r.meeting_id
LEFT JOIN transcripts t ON m.id = t.meeting_id
LEFT JOIN diarizations d ON m.id = d.meeting_id
LEFT JOIN summaries s ON m.id = s.meeting_id
LEFT JOIN (
    SELECT DISTINCT meeting_id, id FROM embeddings
) e ON m.id = e.meeting_id
ORDER BY m.created_at DESC;

-- View: Recent activity summary
CREATE OR REPLACE VIEW v_recent_activity AS
SELECT 
    'recording' as activity_type,
    m.id as meeting_id,
    m.name as meeting_name,
    r.created_at as activity_time,
    r.duration_secs as metric_value,
    'seconds' as metric_unit
FROM recordings r
JOIN meetings m ON r.meeting_id = m.id
WHERE r.created_at >= NOW() - INTERVAL '7 days'

UNION ALL

SELECT 
    'transcript' as activity_type,
    m.id as meeting_id,
    m.name as meeting_name,
    t.created_at as activity_time,
    t.word_count as metric_value,
    'words' as metric_unit
FROM transcripts t
JOIN meetings m ON t.meeting_id = m.id
WHERE t.created_at >= NOW() - INTERVAL '7 days'

UNION ALL

SELECT 
    'summary' as activity_type,
    m.id as meeting_id,
    m.name as meeting_name,
    s.created_at as activity_time,
    LENGTH(s.content) as metric_value,
    'characters' as metric_unit
FROM summaries s
JOIN meetings m ON s.meeting_id = m.id
WHERE s.created_at >= NOW() - INTERVAL '7 days'

ORDER BY activity_time DESC
LIMIT 50;

-- Comments
COMMENT ON VIEW v_meeting_daily_stats IS 'Daily meeting statistics for dashboards';
COMMENT ON VIEW v_speaker_stats_per_meeting IS 'Speaker participation stats per meeting';
COMMENT ON VIEW v_speaker_overall_stats IS 'Aggregate speaker statistics across all meetings';
COMMENT ON VIEW v_summary_type_distribution IS 'Distribution of summary types';
COMMENT ON VIEW v_meeting_completeness IS 'Meeting pipeline completion status';
COMMENT ON VIEW v_recent_activity IS 'Recent activity feed for dashboards';

-- Helper function: Get meeting pipeline funnel
CREATE OR REPLACE FUNCTION get_meeting_pipeline_funnel()
RETURNS TABLE (
    stage TEXT,
    count BIGINT,
    percentage NUMERIC
) AS $$
DECLARE
    total_meetings BIGINT;
BEGIN
    SELECT COUNT(*) INTO total_meetings FROM meetings;
    
    RETURN QUERY
    SELECT 'Total Meetings'::TEXT, COUNT(*), 
           ROUND(COUNT(*) * 100.0 / total_meetings, 2)
    FROM meetings
    
    UNION ALL
    
    SELECT 'With Recordings', COUNT(DISTINCT m.id),
           ROUND(COUNT(DISTINCT m.id) * 100.0 / total_meetings, 2)
    FROM meetings m
    JOIN recordings r ON m.id = r.meeting_id
    
    UNION ALL
    
    SELECT 'With Transcripts', COUNT(DISTINCT m.id),
           ROUND(COUNT(DISTINCT m.id) * 100.0 / total_meetings, 2)
    FROM meetings m
    JOIN transcripts t ON m.id = t.meeting_id
    
    UNION ALL
    
    SELECT 'With Diarizations', COUNT(DISTINCT m.id),
           ROUND(COUNT(DISTINCT m.id) * 100.0 / total_meetings, 2)
    FROM meetings m
    JOIN diarizations d ON m.id = d.meeting_id
    
    UNION ALL
    
    SELECT 'With Summaries', COUNT(DISTINCT m.id),
           ROUND(COUNT(DISTINCT m.id) * 100.0 / total_meetings, 2)
    FROM meetings m
    JOIN summaries s ON m.id = s.meeting_id
    
    UNION ALL
    
    SELECT 'With Embeddings', COUNT(DISTINCT m.id),
           ROUND(COUNT(DISTINCT m.id) * 100.0 / total_meetings, 2)
    FROM meetings m
    JOIN (SELECT DISTINCT meeting_id FROM embeddings) e ON m.id = e.meeting_id;
END;
$$ LANGUAGE plpgsql STABLE;

-- Grant permissions
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO meetily_app;