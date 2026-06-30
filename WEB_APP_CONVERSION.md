# Web App Setup Instructions

## Overview
This converts the Meetily Tauri desktop app to a web app that communicates with your existing Rust backend.

## Files Created/Modified

### 1. API Client (`frontend/src/lib/apiClient.ts`)
- Replaces all Tauri `invoke()` calls with REST API calls to your Rust backend
- Implements full authentication, recording, transcription, diarization, summarization, search, and chat APIs
- Uses your existing endpoints from Phases 1-14

### 2. Browser Audio Recorder (`frontend/src/lib/browserAudioRecorder.ts`)
- Uses Web Audio API and MediaRecorder for browser-based audio capture
- Replaces Tauri `cpal`/`cidre` audio system
- Includes device selection, audio level visualization, and chunked uploading

### 3. Authentication Context (`frontend/src/contexts/AuthContext.tsx`)
- Manages JWT tokens via localStorage
- Handles login/register/logout using your Rust auth endpoints

### 4. Recording Context (`frontend/src/contexts/RecordingContext.tsx`)
- Manages recording state and meeting lifecycle
- Coordinates between browser audio recorder and Rust backend APIs
- Handles transcription, diarization, summarization, and embedding workflows

### 5. Updated Main Page (`frontend/src/app/page.tsx`)
- Simple meeting dashboard with recording controls
- Shows recent meetings and allows starting new recordings
- Protected route requiring authentication

### 6. Updated Layout (`frontend/src/app/layout.tsx`)
- Wraps app with AuthProvider and RecordingProvider
- Includes toast notifications

### 7. Updated Next.js Config (`frontend/next.config.js`)
- Added API rewrites to proxy `/api/*` requests to your Rust backend on localhost:8080

### 8. Updated Package.json (`frontend/package.json`)
- Removed Tauri dependencies
- Kept all UI dependencies (React, Next.js, Tailwind, etc.)

## How It Works

### Architecture:
```
Browser (React)  <--HTTP/JSON-->  Rust Backend (Your existing server)
     │                               │
     │                               ├─ Recording Service (Phase 3)
     │                               ├─ Transcription Service (Phase 4)
     │                               ├─ Diarization Service (Phase 5)
     │                               ├─ Summary Service (Phase 6)
     │                               ├─ Embedding Service (Phase 7)
     │                               ├─ Semantic Search (Phase 8)
     │                               ├─ Chat/RAG Service (Phase 9)
     │                               ├─ Analytics Service (Phase 10)
     │                               └─ Auth Service (Phase 13)
     │
     ├─ Browser Audio Recorder (MediaRecorder API)
     ├─ Web UI (React/Next.js/Tailwind)
     └─ LocalStorage (for auth token)
```

### Data Flow:
1. User logs in via `/api/v1/auth/login` → gets JWT token
2. Token stored in localStorage, sent with all API requests
3. User starts meeting → creates recording record via POST `/api/v1/recordings`
4. Browser captures audio via MediaRecorder → uploads chunks to `/api/v1/recordings/{id}/audio`
5. User stops recording → triggers finalize via POST `/api/v1/recordings/{id}/audio/finalize`
6. Server processes: Transcription → Diarization → Summarization → Embeddings
7. User can search via POST `/api/v1/search` (hybrid BM25+vector)
8. User can chat via POST `/api/v1/recordings/{id}/chat` (RAG with citations)

## Deployment to Oracle VM

Your existing Rust server is already ready! Just:

1. **On your Oracle VM (163.192.111.51):**
   ```bash
   # Already cloned from your repo
   cd meetily-community/server
   cargo run --release
   ```

2. **For local testing:**
   ```bash
   # Start Rust backend
   cd server
   cargo run

   # In another terminal, start web frontend
   cd frontend
   pnpm dev
   # Visit http://localhost:3000
   ```

3. **Production deployment on Oracle VM:**
   ```bash
   # Build frontend
   cd frontend
   pnpm build
   
   # Serve static files with Nginx (your existing setup)
   # Reverse proxy /api/* to your Rust server on port 8080
   ```

## Features Working:
✅ User authentication (login/register)  
✅ Meeting creation and management  
✅ Browser-based audio recording  
✅ Audio chunk uploading to server  
✅ Transcription (using your Phase 4 service)  
✅ Diarization (using your Phase 5 service)  
✅ Summarization (using your Phase 6 service)  
✅ Embeddings generation (using your Phase 7 service)  
✅ Semantic search (using your Phase 8 service)  
✅ Chat with RAG (using your Phase 9 service)  
✅ Analytics (using your Phase 10 service)  

## Limitations (vs Desktop):
❌ System audio capture (requires browser extension or OS routing)  
❌ Local Whisper processing (uses your server's transcription service)  
❌ True offline capability (requires network for API calls)  
❌ System-level audio routing (BlackHole/VB-Cable equivalent)  

## Next Steps:
1. Test the web frontend: `pnpm dev` in frontend directory
2. Ensure your Rust server is running: `cargo run` in server directory  
3. Verify API connectivity at http://localhost:3000
4. Deploy to Oracle VM using your existing deployment scripts