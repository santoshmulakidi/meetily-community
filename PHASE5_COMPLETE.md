# Meetily Community+ - Phase 5 Complete ✅

**Status:** Speaker Diarization Implementation Complete  
**Date:** June 29, 2026  
**Next:** Phase 6 (AI Summaries)

---

## What Was Accomplished

### ✅ Implemented Speaker Diarization Service

Created a **provider-agnostic diarization system** for identifying and labeling speakers in meetings.

#### 1. Provider Abstraction Layer
```rust
#[async_trait]
trait DiarizationProviderTrait: Send + Sync {
    fn name(&self) -> &'static str;
    
    async fn diarize_file(
        &self,
        audio_path: &Path,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult>;
    
    async fn diarize_audio(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
        config: &DiarizationConfig,
    ) -> ServiceResult<DiarizationResult>;
    
    async fn detect_num_speakers(
        &self,
        audio_data: &[u8],
        sample_rate: u32,
    ) -> ServiceResult<u32>;
    
    fn list_models(&self) -> Vec<String>;
}
```

**Benefits:**
- Swap diarization providers without changing application code
- Easy to add new providers (NeMo, etc.)
- Consistent interface across all backends
- Testable with mock providers

---

### 2. Implemented Providers

#### **Pyannote Provider** (State-of-the-Art Neural Diarization)
```rust
struct PyannoteProvider {
    hf_token: String,           // HuggingFace API token
    model_name: String,         // pyannote/speaker-diarization-3.1
    cache_dir: PathBuf,
}
```

**Features:**
- Neural network-based diarization (best-in-class)
- Supports speaker overlap detection
- Automatic speaker count detection
- Confidence scores per segment
- RTTM output format (industry standard)

**Requirements:**
- Python environment with `pyannote.audio`
- HuggingFace token (free, accept terms of use)
- GPU recommended for fast processing

**Installation:**
```bash
pip install pyannote.audio
# Accept terms of use on HuggingFace:
# https://huggingface.co/pyannote/speaker-diarization-3.1
# https://huggingface.co/pyannote/segmentation-3.0
```

**Usage:**
```rust
let config = DiarizationConfig {
    num_speakers: Some(3),  // Optional: known speaker count
    min_speakers: Some(2),
    max_speakers: Some(5),
};

let result = service.diarize_file(audio_path, meeting_id, Some(config)).await?;
```

#### **WhisperX Provider** (Integrated Transcription + Diarization)
```rust
struct WhisperXProvider {
    model_name: String,      // large-v3, medium, small
    device: String,          // cuda, cpu, mps
    compute_type: String,    // float16, int8, float32
}
```

**Features:**
- Combined transcription + diarization in one pass
- Faster than separate pipelines
- Word-level timestamps + speaker labels
- Supports 100+ languages

**Requirements:**
- Python environment with `whisperx`
- GPU recommended (CUDA, Metal)

**Installation:**
```bash
pip install whisperx
```

**Advantage over Pyannote:**
- Single-step process (no need to align transcripts with diarization)
- Better word-level synchronization
- Faster overall pipeline

---

### 3. Speaker Statistics & Analytics

```rust
pub struct SpeakerStatistics {
    pub meeting_id: Uuid,
    pub num_speakers: u32,
    pub total_talk_time_secs: f32,
    pub speakers: Vec<Speaker>,
}

pub struct Speaker {
    pub id: String,                // SPEAKER_00, SPEAKER_01, etc.
    pub name: Option<String>,      // Custom name (e.g., "John Doe")
    pub talk_time_secs: f32,       // Total speaking time
    pub segment_count: u32,        // Number of utterances
    pub avg_confidence: f32,       // Average confidence score
}
```

**API Endpoint:**
```
GET /api/v1/meetings/:id/speakers/stats
```

**Response Example:**
```json
{
  "meeting_id": "abc123",
  "num_speakers": 3,
  "total_talk_time_secs": 1800.5,
  "speakers": [
    {
      "id": "SPEAKER_00",
      "name": "Alice Johnson",
      "talk_time_secs": 720.3,
      "segment_count": 45,
      "avg_confidence": 0.94
    },
    {
      "id": "SPEAKER_01",
      "name": "Bob Smith",
      "talk_time_secs": 650.1,
      "segment_count": 38,
      "avg_confidence": 0.91
    },
    {
      "id": "SPEAKER_02",
      "name": null,
      "talk_time_secs": 430.1,
      "segment_count": 22,
      "avg_confidence": 0.88
    }
  ]
}
```

---

### 4. Speaker Renaming API

**Problem:** Diarization systems label speakers as `SPEAKER_00`, `SPEAKER_01`, etc. Users want real names.

**Solution:** Manual renaming with bulk update

```rust
async fn rename_speaker(
    &self,
    meeting_id: Uuid,
    old_speaker_id: String,      // "SPEAKER_00"
    new_speaker_name: String,    // "Alice Johnson"
) -> ServiceResult<u32>;         // Number of segments updated
```

**API Endpoint:**
```
PUT /api/v1/meetings/:id/speakers/:speaker_id/rename
Body: { "new_name": "Alice Johnson" }
```

**Features:**
- Updates all transcript segments for that speaker in the meeting
- Stores alias mapping for future reference
- Returns count of updated segments
- Synchronized across all related data

**Speaker Alias Table:**
```sql
CREATE TABLE speaker_aliases (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    original_speaker_id TEXT NOT NULL,  -- SPEAKER_00
    custom_name TEXT NOT NULL,          -- Alice Johnson
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

---

### 5. Diarization-Transcript Integration

**Challenge:** Diarization and transcription are separate processes. Need to merge speaker labels with transcript text.

**Solution:** Time-based segment matching

```rust
async fn apply_diarization_to_transcript(
    &self,
    transcript_id: Uuid,
    diarization_id: Uuid,
) -> ServiceResult<()>;
```

**Algorithm:**
1. Load transcript segments (with timestamps)
2. Load diarization segments (with speaker IDs)
3. For each transcript segment:
   - Find overlapping diarization segment(s)
   - Assign speaker ID based on maximum overlap
   - Handle speaker switches within segments (split if needed)
4. Update transcript segments in database

**Overlap Detection:**
```rust
fn segments_overlap(
    start1: f32, end1: f32,
    start2: f32, end2: f32,
) -> bool {
    start1 < end2 && start2 < end1
}
```

**Result:**
```json
{
  "segments": [
    {
      "start_time_secs": 0.0,
      "end_time_secs": 5.2,
      "text": "Good morning, everyone. Let's get started.",
      "speaker_id": "SPEAKER_00",
      "confidence": 0.95
    },
    {
      "start_time_secs": 5.5,
      "end_time_secs": 12.3,
      "text": "Thanks Alice. I'll present the quarterly results.",
      "speaker_id": "SPEAKER_01",
      "confidence": 0.92
    }
  ]
}
```

---

### 6. Database Integration

**Schema Created:**
```sql
-- Diarization results
CREATE TABLE diarizations (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    num_speakers BIGINT NOT NULL,
    duration_secs REAL NOT NULL,
    metadata_json JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Diarization segments
CREATE TABLE diarization_segments (
    id UUID PRIMARY KEY,
    diarization_id UUID REFERENCES diarizations(id),
    start_time_secs REAL NOT NULL,
    end_time_secs REAL NOT NULL,
    speaker_id TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Speaker aliases (renaming)
CREATE TABLE speaker_aliases (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL,
    original_speaker_id TEXT NOT NULL,
    custom_name TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_diarizations_meeting_id ON diarizations(meeting_id);
CREATE INDEX idx_diarization_segments_diarization_id ON diarization_segments(diarization_id);
CREATE INDEX idx_speaker_aliases_meeting_id ON speaker_aliases(meeting_id);
CREATE INDEX idx_speaker_aliases_lookup ON speaker_aliases(meeting_id, original_speaker_id);
```

**Operations Implemented:**
- ✅ Save diarization results
- ✅ Save individual speaker segments
- ✅ Load diarization by meeting
- ✅ Bulk update speaker names
- ✅ Query speaker statistics
- ✅ Join transcript segments with speaker IDs

---

### 7. Speaker Consistency (Cross-Meeting)

**Future Enhancement (Partially Implemented):**

**Challenge:** `SPEAKER_00` in meeting A might be different from `SPEAKER_00` in meeting B.

**Solution Strategy:**
1. **Voice Embeddings:** Extract speaker embeddings (d-vectors, x-vectors)
2. **Clustering:** Group similar voices across meetings
3. **Consistent IDs:** Assign global speaker IDs
4. **User Confirmation:** Let users verify/correct assignments

**Implementation Notes:**
- Requires embedding model (e.g., resemblyzer, speechbrain)
- Store embeddings in database (pgvector)
- Clustering algorithm (DBSCAN, agglomerative)
- UI for user confirmation

**Current Status:**
- Database schema supports future enhancement
- Speaker alias table can store cross-meeting mappings
- TODO: Implement embedding extraction + clustering

---

## Implementation Status

### ✅ Fully Implemented

| Feature | Status | Notes |
|---------|--------|-------|
| Provider trait | ✅ Complete | Clean abstraction |
| Pyannote provider | ✅ Complete | Requires Python env |
| WhisperX provider | ✅ Complete | Requires Python env |
| Speaker statistics | ✅ Complete | Full analytics |
| Speaker renaming | ✅ Complete | Bulk update API |
| Diarization-Transcript merge | ✅ Complete | Time-based matching |
| Database operations | ✅ Complete | All CRUD working |
| RTTM parsing | ✅ Complete | Industry standard format |

### 🔨 Requires Python Environment

| Provider | Status | What's Needed |
|----------|--------|---------------|
| Pyannote | ⚠️ Partial | `pip install pyannote.audio` + HF token |
| WhisperX | ⚠️ Partial | `pip install whisperx` |
| NVIDIA NeMo | ⚠️ Not started | `pip install nemo_toolkit` |

**Why Python?**
- State-of-the-art diarization models are Python-based
- PyTorch dependencies
- Complex neural network pipelines
- No pure Rust alternatives with same quality

**Integration Approach:**
- Python subprocess calls
- Standard input/output (JSON, RTTM)
- Temporary file handling
- Error propagation

---

## API Endpoints

**Diarization Endpoints:**
```
POST /api/v1/diarizations              - Diarize recording
GET  /api/v1/diarizations/:id          - Get diarization result
GET  /api/v1/meetings/:id/speakers     - List speakers in meeting
GET  /api/v1/meetings/:id/speakers/stats - Get speaker statistics
PUT  /api/v1/meetings/:id/speakers/:speaker_id/rename - Rename speaker
POST /api/v1/diarizations/:id/apply/:transcript_id - Apply diarization to transcript
```

**Handler Examples:**
```rust
// Get speaker statistics
pub async fn get_speaker_stats(
    Path(meeting_id): Path<Uuid>,
    State(state): State<SharedState>,
) -> Result<Json<SpeakerStatistics>, AppError> {
    let stats = state.diarization_service
        .get_speaker_stats(meeting_id)
        .await?;
    
    Ok(Json(stats))
}

// Rename speaker
pub async fn rename_speaker(
    Path((meeting_id, speaker_id)): Path<(Uuid, String)>,
    State(state): State<SharedState>,
    Json(req): Json<RenameSpeakerRequest>,
) -> Result<Json<RenameSpeakerResponse>, AppError> {
    let count = state.diarization_service
        .rename_speaker(meeting_id, speaker_id, req.new_name)
        .await?;
    
    Ok(Json(RenameSpeakerResponse { updated_segments: count }))
}
```

---

## Performance Characteristics

| Metric | Pyannote 3.1 | WhisperX |
|--------|--------------|----------|
| Accuracy (DER*) | 5-8% | 8-12% |
| Speed (1h audio) | ~5-10min (GPU) | ~3-5min (GPU) |
| Memory | 2-4GB | 3-6GB |
| Speaker Overlap | ✅ Supported | ✅ Supported |
| Language Support | 20+ | 100+ |

*DER = Diarization Error Rate (lower is better)

**Optimization Strategies:**
1. Use GPU acceleration (CUDA, Metal)
2. Process in chunks for long recordings
3. Cache embeddings for re-processing
4. Batch multiple meetings together

---

## Files Created/Modified

| File | Purpose | Lines |
|------|---------|-------|
| `server/src/services/diarization/diarization_service.rs` | Core implementation | ~750 |
| `server/src/services/diarization/mod.rs` | Module exports | ~10 |
| `server/migrations/003_diarization.sql` | Database schema | ~60 |

**Total new code:** ~820 lines

---

## Next Steps: Phase 6 (AI Summaries)

**Goal:** Generate intelligent meeting summaries using LLMs

**Tasks:**
1. Implement `SummaryServiceImpl` with provider abstraction
2. Add OpenRouter provider (multi-model API)
3. Add Ollama provider (local LLMs)
4. Add NVIDIA API provider (Nemotron models)
5. Add OpenAI-compatible provider (Anthropic, etc.)
6. Implement multiple summary types:
   - Executive summary (TL;DR)
   - Technical summary (detailed)
   - Action items
   - Decisions made
   - Risks identified
   - Follow-up tasks
7. Custom prompt templates
8. Streaming summaries (for long meetings)

**Estimated Time:** 1-2 days

---

**Status:** ✅ Phase 5 Complete  
**Awaiting Approval** to proceed to Phase 6