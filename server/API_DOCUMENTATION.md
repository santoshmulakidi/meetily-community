# Meetily Community+ API Documentation

**Version:** 1.0.0  
**Base URL:** `http://localhost:8080/api/v1`  
**OpenAPI Spec:** `/api/v1/openapi.json`  
**Swagger UI:** `/swagger-ui`

---

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Endpoints](#endpoints)
   - [Health](#health-endpoints)
   - [Meetings](#meeting-endpoints)
   - [Recordings](#recording-endpoints)
   - [Transcripts](#transcript-endpoints)
   - [Diarizations](#diarization-endpoints)
   - [Summaries](#summary-endpoints)
   - [Embeddings](#embedding-endpoints)
   - [Chat](#chat-endpoints)
   - [Analytics](#analytics-endpoints)
4. [Error Handling](#error-handling)
5. [Rate Limiting](#rate-limiting)
6. [Examples](#examples)
7. [Changelog](#changelog)

---

## Overview

Meetily Community+ is a self-hosted AI meeting assistant that transforms audio meetings into actionable insights using state-of-the-art machine learning.

### Key Features

- **🎙️ Recording** - Unlimited duration audio recording with file rotation and crash recovery
- **📝 Transcription** - Multi-provider speech-to-text (Whisper, NVIDIA, faster-whisper)
- **👥 Diarization** - Automatic speaker identification and labeling
- **📊 Summaries** - AI-generated summaries in 6 formats
- **🧠 Embeddings** - Vector embeddings for semantic search
- **💬 Chat** - ChatGPT-style Q&A with citations
- **📈 Analytics** - Meeting statistics and insights

### Technology Stack

- **Backend:** Rust 2021 with Axum web framework
- **Database:** PostgreSQL 16 with pgvector extension
- **API:** RESTful with OpenAPI 3.0 specification
- **Authentication:** JWT-based Bearer tokens

---

## Authentication

Most endpoints require authentication via Bearer token in the `Authorization` header:

```http
Authorization: Bearer <your-api-key>
```

### Public Endpoints (No Auth Required)

- `GET /health` - Health check
- `GET /health/detailed` - Detailed health with dependencies
- `GET /api/v1/openapi.json` - OpenAPI specification
- `GET /swagger-ui` - Interactive API documentation

### Obtaining an API Key

**Note:** Authentication is not yet implemented in this version. Currently, all endpoints are accessible without authentication.

Future versions will include:
- OAuth 2.0 authentication
- API key management
- User registration and login
- Role-based access control

---

## Endpoints

### Health Endpoints

#### `GET /health`

Basic health check to verify the server is running.

**Response:**
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "timestamp": "2024-06-29T14:30:00Z"
}
```

---

#### `GET /health/detailed`

Detailed health check showing status of all dependencies.

**Response:**
```json
{
  "status": "healthy",
  "database": "connected",
  "storage": "available",
  "providers": [
    "nvidia/transcription",
    "nvidia/embedding",
    "openrouter/summary"
  ]
}
```

---

### Meeting Endpoints

#### `POST /api/v1/meetings`

Create a new meeting.

**Request:**
```json
{
  "name": "Weekly Team Standup",
  "description": "Weekly sync with the engineering team",
  "participants": ["alice@company.com", "bob@company.com"]
}
```

**Response (201 Created):**
```json
{
  "id": "uuid",
  "name": "Weekly Team Standup",
  "description": "Weekly sync with the engineering team",
  "participants": ["alice@company.com", "bob@company.com"],
  "created_at": "2024-06-29T14:30:00Z",
  "updated_at": "2024-06-29T14:30:00Z"
}
```

---

#### `GET /api/v1/meetings`

List all meetings with pagination.

**Query Parameters:**
- `limit` (default: 20) - Number of results
- `offset` (default: 0) - Pagination offset
- `from` - Filter by start date (ISO 8601)
- `to` - Filter by end date (ISO 8601)

**Response:**
```json
{
  "meetings": [...],
  "total": 150,
  "limit": 20,
  "offset": 0
}
```

---

#### `GET /api/v1/meetings/:id`

Get a specific meeting by ID.

**Response:**
```json
{
  "id": "uuid",
  "name": "Weekly Team Standup",
  "description": "...",
  "participants": [...],
  "recordings": [...],
  "transcripts": [...],
  "summaries": [...]
}
```

---

#### `PUT /api/v1/meetings/:id`

Update a meeting's metadata.

**Request:**
```json
{
  "name": "Updated Meeting Name",
  "description": "Updated description"
}
```

---

#### `DELETE /api/v1/meetings/:id`

Delete a meeting and all associated data (recordings, transcripts, summaries, embeddings).

**Response (204 No Content)**

---

### Recording Endpoints

#### `POST /api/v1/recordings/start`

Start a new audio recording session.

**Request:**
```json
{
  "meeting_id": "uuid",
  "duration_secs": 3600
}
```

**Response (201 Created):**
```json
{
  "id": "uuid",
  "session_id": "session-uuid",
  "meeting_id": "uuid",
  "status": "recording",
  "started_at": "2024-06-29T14:30:00Z"
}
```

---

#### `POST /api/v1/recordings/:id/stop`

Stop an active recording session.

**Response:**
```json
{
  "id": "uuid",
  "status": "completed",
  "duration_secs": 3542,
  "file_path": "/path/to/recording.wav",
  "file_size_bytes": 125000000
}
```

---

#### `POST /api/v1/recordings/:id/pause`

Pause an active recording (for breaks).

**Response:**
```json
{
  "id": "uuid",
  "status": "paused",
  "paused_at": "2024-06-29T15:00:00Z"
}
```

---

#### `POST /api/v1/recordings/:id/resume`

Resume a paused recording.

**Response:**
```json
{
  "id": "uuid",
  "status": "recording",
  "resumed_at": "2024-06-29T15:10:00Z"
}
```

---

#### `GET /api/v1/recordings/:id`

Get recording metadata and status.

**Response:**
```json
{
  "id": "uuid",
  "meeting_id": "uuid",
  "status": "completed",
  "duration_secs": 3542,
  "file_path": "/path/to/recording.wav",
  "file_size_bytes": 125000000,
  "created_at": "2024-06-29T14:30:00Z"
}
```

---

#### `GET /api/v1/recordings`

List recordings with optional meeting filter.

**Query Parameters:**
- `meeting_id` - Filter by meeting
- `status` - Filter by status (recording, completed, failed)
- `limit`, `offset` - Pagination

**Response:**
```json
{
  "recordings": [...],
  "total": 45
}
```

---

### Transcript Endpoints

#### `GET /api/v1/transcripts/:meeting_id`

Get the transcript for a specific meeting.

**Response:**
```json
{
  "meeting_id": "uuid",
  "content": "Full transcript text...",
  "segments": [
    {
      "id": "uuid",
      "speaker_id": "SPEAKER_00",
      "speaker_name": "Alice",
      "text": "Good morning everyone...",
      "start_time_secs": 0.0,
      "end_time_secs": 5.2,
      "confidence": 0.95
    }
  ],
  "language": "en",
  "duration_secs": 3542,
  "created_at": "2024-06-29T14:30:00Z"
}
```

---

#### `POST /api/v1/transcripts/:meeting_id/generate`

Generate or regenerate transcript from recording.

**Request:**
```json
{
  "provider": "nvidia",  // nvidia, whisper, faster-whisper
  "language": "en",      // auto-detect if not specified
  "model": "parakeet-0.6b"
}
```

**Response (202 Accepted):**
```json
{
  "status": "processing",
  "job_id": "job-uuid",
  "estimated_time_secs": 120
}
```

---

#### `GET /api/v1/transcripts/:meeting_id/segments`

Get transcript segments with pagination.

**Query Parameters:**
- `limit` (default: 50)
- `offset` (default: 0)
- `speaker_id` - Filter by speaker

**Response:**
```json
{
  "segments": [...],
  "total": 450,
  "limit": 50,
  "offset": 0
}
```

---

### Diarization Endpoints

#### `POST /api/v1/diarizations/:meeting_id/generate`

Generate speaker diarization for a meeting.

**Request:**
```json
{
  "provider": "pyannote",  // pyannote, whisperx
  "num_speakers": 5        // optional, auto-detect if not specified
}
```

**Response (202 Accepted):**
```json
{
  "status": "processing",
  "job_id": "job-uuid"
}
```

---

#### `GET /api/v1/diarizations/:meeting_id`

Get diarization results for a meeting.

**Response:**
```json
{
  "meeting_id": "uuid",
  "speakers": [
    {
      "speaker_id": "SPEAKER_00",
      "alias": "Alice",
      "total_talk_time_secs": 1850,
      "segment_count": 125
    },
    {
      "speaker_id": "SPEAKER_01",
      "alias": "Bob",
      "total_talk_time_secs": 1420,
      "segment_count": 98
    }
  ],
  "segments": [...],
  "created_at": "2024-06-29T14:30:00Z"
}
```

---

#### `PUT /api/v1/diarizations/:meeting_id/speakers/:speaker_id`

Update speaker alias (rename SPEAKER_00 to "Alice").

**Request:**
```json
{
  "alias": "Alice Johnson"
}
```

**Response:**
```json
{
  "speaker_id": "SPEAKER_00",
  "alias": "Alice Johnson",
  "updated": true
}
```

---

### Summary Endpoints

#### `POST /api/v1/summaries/:meeting_id/generate`

Generate AI summary for a meeting.

**Request:**
```json
{
  "summary_type": "executive",  // executive, technical, action_items, decisions, risks, follow_up
  "provider": "openrouter",     // openrouter, ollama, nvidia, openai
  "model": "meta-llama/llama-3.1-70b-instruct",
  "custom_prompt": null         // optional custom prompt
}
```

**Response (202 Accepted):**
```json
{
  "status": "processing",
  "job_id": "job-uuid",
  "estimated_time_secs": 30
}
```

---

#### `GET /api/v1/summaries/:meeting_id`

Get all summaries for a meeting.

**Response:**
```json
{
  "meeting_id": "uuid",
  "summaries": [
    {
      "id": "uuid",
      "summary_type": "executive",
      "content": "TL;DR: The team discussed the API redesign...",
      "model_used": "llama-3.1-70b",
      "provider": "openrouter",
      "token_usage": {
        "prompt_tokens": 2500,
        "completion_tokens": 450,
        "total_tokens": 2950,
        "cost_usd": 0.012
      },
      "created_at": "2024-06-29T14:35:00Z"
    },
    {
      "summary_type": "action_items",
      "content": "1. Alice: Review Docker proposal by Friday...",
      ...
    }
  ]
}
```

---

#### `GET /api/v1/summaries/:meeting_id/:type`

Get a specific summary type for a meeting.

**Path Parameters:**
- `type` - executive, technical, action_items, decisions, risks, follow_up

**Response:**
```json
{
  "id": "uuid",
  "meeting_id": "uuid",
  "summary_type": "executive",
  "content": "...",
  ...
}
```

---

### Embedding Endpoints

#### `POST /api/v1/embeddings/meetings/:id`

Generate embeddings for a meeting transcript (enables semantic search).

**Request:**
```json
{
  "chunking_strategy": "fixed_size",  // fixed_size, semantic, speaker_turn
  "chunk_size": 512,
  "overlap": 50,
  "provider": "nvidia",               // nvidia, openai, local
  "model": "nvidia/nv-embedqa-e5-v5"
}
```

**Response (202 Accepted):**
```json
{
  "status": "processing",
  "job_id": "job-uuid",
  "estimated_embeddings": 45
}
```

---

#### `GET /api/v1/embeddings/search`

Semantic search across meetings.

**Query Parameters:**
- `q` - Search query (required)
- `meeting_id` - Limit search to specific meeting
- `limit` - Number of results (default: 10)
- `threshold` - Minimum similarity score (0.0-1.0, default: 0.5)

**Response:**
```json
{
  "query": "Docker deployment issues",
  "results": [
    {
      "meeting_id": "uuid",
      "meeting_name": "Infrastructure Meeting",
      "chunk_text": "We discussed Docker deployment issues in production...",
      "speaker_id": "SPEAKER_00",
      "speaker_name": "Alice",
      "start_time_secs": 930.0,
      "end_time_secs": 960.0,
      "similarity": 0.89
    }
  ],
  "total": 15
}
```

---

### Chat Endpoints

#### `POST /api/v1/chat`

Send a message to the AI chat assistant.

**Request:**
```json
{
  "conversation_id": null,          // null for new conversation
  "meeting_ids": ["uuid1", "uuid2"], // Context meetings (null = all)
  "message": "What decisions were made about Docker?",
  "config": {
    "max_context_chunks": 10,
    "temperature": 0.3
  }
}
```

**Response:**
```json
{
  "conversation_id": "generated-uuid",
  "message": "Based on the meetings, three key decisions were made:\n\n1. Use Docker containers for microservices [Meeting: ABC Corp, 15:30-16:00, Speaker: Alice]\n2. Implement health checks for all containers [Meeting: ABC Corp, 18:30-19:00, Speaker: Bob]\n3. Schedule migration to Kubernetes in Q3 [Meeting: XYZ Inc, 22:15-22:45, Speaker: Charlie]",
  "citations": [
    {
      "meeting_id": "uuid1",
      "transcript_id": "transcript-uuid",
      "start_time_secs": 930.0,
      "end_time_secs": 960.0,
      "speaker_id": "SPEAKER_00",
      "speaker_name": "Alice",
      "text": "We decided to use Docker...",
      "relevance_score": 0.92
    }
  ],
  "context_used": 5,
  "model_used": "llama-3.1-70b",
  "created_at": "2024-06-29T14:30:00Z"
}
```

---

#### `GET /api/v1/chat/:conversation_id`

Get conversation history.

**Response:**
```json
{
  "id": "uuid",
  "meeting_ids": ["uuid1", "uuid2"],
  "messages": [
    {
      "role": "user",
      "content": "What decisions were made about Docker?",
      "timestamp": "2024-06-29T14:30:00Z"
    },
    {
      "role": "assistant",
      "content": "Based on the meetings...",
      "timestamp": "2024-06-29T14:30:05Z"
    }
  ],
  "created_at": "2024-06-29T14:30:00Z",
  "updated_at": "2024-06-29T14:30:05Z"
}
```

---

#### `GET /api/v1/chat/conversations`

List all conversations.

**Query Parameters:**
- `limit` (default: 20)
- `offset` (default: 0)

**Response:**
```json
{
  "conversations": [...],
  "total": 45
}
```

---

#### `DELETE /api/v1/chat/:conversation_id`

Delete a conversation and all its messages.

**Response (204 No Content)**

---

### Analytics Endpoints

#### `GET /api/v1/analytics/dashboard`

Get comprehensive dashboard with all analytics.

**Query Parameters:**
- `range` - Time range: last_7_days, last_30_days, last_90_days

**Response:**
```json
{
  "meeting_stats": {
    "total_meetings": 150,
    "total_hours": 87.5,
    "avg_duration_mins": 35,
    "with_transcripts": 142,
    "with_summaries": 138,
    "daily_trend": {
      "2024-06-01": 5,
      "2024-06-02": 3,
      ...
    }
  },
  "speaker_stats": [
    {
      "speaker_id": "SPEAKER_00",
      "speaker_name": "Alice Johnson",
      "talk_time_percentage": 42.5,
      "meetings_participated": 15
    }
  ],
  "topic_analytics": {
    "top_topics": [
      ["docker", 342],
      ["api", 287],
      ["kubernetes", 198]
    ],
    "summary_type_distribution": {
      "executive": 45,
      "action_items": 42,
      ...
    }
  },
  "sentiment_summary": {
    "overall_sentiment": "neutral",
    "sentiment_score": 0.5
  },
  "action_items": {
    "total_items": 200,
    "completed_items": 120,
    "completion_rate": 0.6
  },
  "generated_at": "2024-06-29T14:30:00Z"
}
```

---

#### `GET /api/v1/analytics/meetings`

Get meeting statistics only.

**Response:** See `meeting_stats` in dashboard response.

---

#### `GET /api/v1/analytics/speakers`

Get speaker analytics.

**Query Parameters:**
- `meeting_id` - Filter to specific meeting (optional)

**Response:**
```json
[
  {
    "speaker_id": "SPEAKER_00",
    "speaker_name": "Alice Johnson",
    "segment_count": 125,
    "total_talk_time_secs": 1850.5,
    "avg_confidence": 0.94,
    "meetings_participated": 15,
    "talk_time_percentage": 42.5
  }
]
```

---

#### `GET /api/v1/analytics/topics`

Get topic analytics (most discussed topics).

**Response:**
```json
{
  "top_topics": [["docker", 342], ["api", 287], ...],
  "summary_type_distribution": {...},
  "topic_evolution": {...}
}
```

---

#### `GET /api/v1/analytics/action-items`

Get action item statistics.

**Response:**
```json
{
  "total_items": 200,
  "completed_items": 120,
  "pending_items": 80,
  "overdue_items": 30,
  "completion_rate": 0.6,
  "top_owners": [...]
}
```

---

#### `GET /api/v1/analytics/trends`

Get meeting trends over time.

**Query Parameters:**
- `days` - Number of days (default: 30)

**Response:**
```json
[
  {
    "date": "2024-06-01",
    "meeting_count": 5,
    "total_duration_mins": 175,
    "transcripts_count": 5,
    "summaries_count": 5
  },
  {
    "date": "2024-06-02",
    "meeting_count": 3,
    "total_duration_mins": 105,
    ...
  }
]
```

---

## Error Handling

All errors return a standard JSON format:

```json
{
  "error": "ErrorType",
  "message": "Human-readable error message",
  "details": {
    "field": "value"
  }
}
```

### Common Error Codes

| HTTP Status | Error Type | Description |
|-------------|------------|-------------|
| 400 | `BadRequest` | Invalid request format or parameters |
| 401 | `Unauthorized` | Missing or invalid authentication |
| 403 | `Forbidden` | Insufficient permissions |
| 404 | `NotFound` | Resource not found |
| 409 | `Conflict` | Resource already exists |
| 422 | `ValidationError` | Request validation failed |
| 429 | `RateLimitExceeded` | Too many requests |
| 500 | `InternalServerError` | Server error |
| 503 | `ServiceUnavailable` | Service temporarily unavailable |

### Example Error Response

```json
{
  "error": "NotFound",
  "message": "Meeting with ID 'abc-123' not found",
  "details": {
    "meeting_id": "abc-123"
  }
}
```

---

## Rate Limiting

**Current Status:** Rate limiting is not yet implemented in this version.

Future implementation will include:
- 100 requests per minute per API key
- Rate limit headers in responses:
  - `X-RateLimit-Limit`: Maximum requests allowed
  - `X-RateLimit-Remaining`: Requests remaining
  - `X-RateLimit-Reset`: Time when limit resets

---

## Examples

### Complete Workflow: Record → Transcribe → Summarize → Chat

```bash
# 1. Create a meeting
curl -X POST http://localhost:8080/api/v1/meetings \
  -H "Content-Type: application/json" \
  -d '{"name": "Team Standup", "participants": ["alice@company.com"]}'

# 2. Start recording
curl -X POST http://localhost:8080/api/v1/recordings/start \
  -H "Content-Type: application/json" \
  -d '{"meeting_id": "uuid", "duration_secs": 3600}'

# 3. Stop recording after meeting
curl -X POST http://localhost:8080/api/v1/recordings/:id/stop

# 4. Generate transcript
curl -X POST http://localhost:8080/api/v1/transcripts/:meeting_id/generate \
  -H "Content-Type: application/json" \
  -d '{"provider": "nvidia"}'

# Wait for transcription to complete...

# 5. Generate summary
curl -X POST http://localhost:8080/api/v1/summaries/:meeting_id/generate \
  -H "Content-Type: application/json" \
  -d '{"summary_type": "executive", "provider": "openrouter"}'

# 6. Chat about the meeting
curl -X POST http://localhost:8080/api/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "meeting_ids": ["uuid"],
    "message": "What action items were assigned to Alice?"
  }'

# 7. Search semantically
curl "http://localhost:8080/api/v1/embeddings/search?q=Docker+deployment&meeting_id=uuid"
```

---

## Changelog

### v1.0.0 (2024-06-29) - Initial Release

**Features:**
- ✅ Recording service with crash recovery
- ✅ Multi-provider transcription (NVIDIA, Whisper, faster-whisper)
- ✅ Speaker diarization with renaming
- ✅ AI summaries (6 types, 4 LLM providers)
- ✅ Vector embeddings with pgvector
- ✅ ChatGPT-style chat with RAG
- ✅ Comprehensive analytics dashboard
- ✅ OpenAPI 3.0 documentation with Swagger UI

**Known Limitations:**
- Authentication not yet implemented (all endpoints public)
- Rate limiting not implemented
- Streaming responses not implemented
- Local embedding provider requires Python environment

### Planned Features (v1.1.0)

- [ ] JWT-based authentication
- [ ] User registration and login
- [ ] API key management
- [ ] Rate limiting
- [ ] Streaming chat responses (SSE)
- [ ] Webhook notifications
- [ ] Bulk operations
- [ ] Export meetings to PDF/Markdown

### Planned Features (v2.0.0)

- [ ] Multi-user workspaces
- [ ] Role-based access control
- [ ] Meeting sharing and collaboration
- [ ] Calendar integration (Google, Outlook)
- [ ] Real-time transcription (WebSocket)
- [ ] Mobile app (React Native)
- [ ] Browser extension (Chrome, Firefox)

---

## Support

**GitHub:** https://github.com/Zackriya-Solutions/meetily  
**Issues:** https://github.com/Zackriya-Solutions/meetily/issues  
**Documentation:** https://hermes-agent.nousresearch.com/docs

## License

MIT License - See [LICENSE](../LICENSE) for details.