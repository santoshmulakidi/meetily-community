/**
 * Web API Client for Meetily
 * 
 * Replaces Tauri invoke() calls with REST API calls to the Rust backend.
 * Provides the same interface as the Tauri services but uses HTTP.
 * 
 * Usage:
 *   import { apiClient } from '@/lib/apiClient';
 *   const recordings = await apiClient.recordings.list();
 */

const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://163.192.111.51:8082/api/v1';

interface RequestOptions extends RequestInit {
  params?: Record<string, string | number | boolean | undefined>;
}

class ApiClient {
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl: string = API_BASE) {
    this.baseUrl = baseUrl;
    // Load token from localStorage on initialization
    if (typeof window !== 'undefined') {
      this.token = localStorage.getItem('auth_token');
    }
  }

  setToken(token: string | null) {
    this.token = token;
    if (typeof window !== 'undefined') {
      if (token) {
        localStorage.setItem('auth_token', token);
      } else {
        localStorage.removeItem('auth_token');
      }
    }
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  private buildUrl(endpoint: string, params?: Record<string, any>): string {
    const url = new URL(`${this.baseUrl}${endpoint}`, window.location.origin);
    if (params) {
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.append(key, String(value));
        }
      });
    }
    return url.toString();
  }

  private async request<T>(endpoint: string, options: RequestOptions = {}): Promise<T> {
    const { params, headers, ...fetchOptions } = options;
    
    const url = this.buildUrl(endpoint, params);
    
    const defaultHeaders: HeadersInit = {
      'Content-Type': 'application/json',
      ...(this.token && { 'Authorization': `Bearer ${this.token}` }),
      ...headers,
    };

    const response = await fetch(url, {
      ...fetchOptions,
      headers: defaultHeaders,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: 'Request failed' }));
      throw new ApiError(response.status, error.message || 'Request failed');
    }

    // Handle 204 No Content
    if (response.status === 204) {
      return undefined as T;
    }

    return response.json();
  }

  // Authentication
  auth = {
    register: (data: { email: string; password: string; full_name: string }) =>
      this.request<{ user: User; token: string }>('/auth/register', {
        method: 'POST',
        body: JSON.stringify(data),
      }),

    login: (data: { email: string; password: string }) =>
      this.request<{ user: User; token: string }>('/auth/login', {
        method: 'POST',
        body: JSON.stringify(data),
      }),

    logout: () =>
      this.request('/auth/logout', { method: 'POST' }),

    me: () =>
      this.request<User>('/auth/me'),

    refreshToken: () =>
      this.request<{ token: string }>('/auth/refresh', { method: 'POST' }),

    changePassword: (data: { current_password: string; new_password: string }) =>
      this.request('/auth/password', {
        method: 'PUT',
        body: JSON.stringify(data),
      }),
  };

  // Recordings
  recordings = {
    list: (params?: { page?: number; limit?: number; search?: string }) =>
      this.request<PaginatedResponse<Recording>>('/recordings', { params }),

    get: (id: string) =>
      this.request<Recording>(`/recordings/${id}`),

    create: (data: { meeting_name: string; device_config?: DeviceConfig }) =>
      this.request<Recording>('/recordings', {
        method: 'POST',
        body: JSON.stringify(data),
      }),

    update: (id: string, data: Partial<Recording>) =>
      this.request<Recording>(`/recordings/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data),
      }),

    delete: (id: string) =>
      this.request(`/recordings/${id}`, { method: 'DELETE' }),

    getTranscripts: (id: string) =>
      this.request<Transcript[]>(`/recordings/${id}/transcripts`),

    getDiarization: (id: string) =>
      this.request<DiarizationResult>(`/recordings/${id}/diarization`),

    getSummaries: (id: string) =>
      this.request<Summary[]>(`/recordings/${id}/summaries`),

    getAnalytics: (id: string) =>
      this.request<MeetingAnalytics>(`/recordings/${id}/analytics`),

    // Audio file upload
    uploadAudio: (recordingId: string, audioBlob: Blob, chunkIndex: number, totalChunks: number) => {
      const formData = new FormData();
      formData.append('audio', audioBlob, `chunk_${chunkIndex}.webm`);
      formData.append('chunk_index', String(chunkIndex));
      formData.append('total_chunks', String(totalChunks));
      
      return this.request<{ status: string; uploaded_chunks: number }>(
        `/recordings/${recordingId}/audio`,
        {
          method: 'POST',
          headers: {
            'Authorization': `Bearer ${this.token}`,
            // Don't set Content-Type - let browser set it with boundary for FormData
          },
          body: formData,
        }
      );
    },

    finalizeAudio: (recordingId: string) =>
      this.request(`/recordings/${recordingId}/audio/finalize`, { method: 'POST' }),
  };

  // Transcription
  transcription = {
    start: (recordingId: string, config?: TranscriptionConfig) =>
      this.request<TranscriptionJob>(`/recordings/${recordingId}/transcribe`, {
        method: 'POST',
        body: JSON.stringify(config || {}),
      }),

    getStatus: (recordingId: string) =>
      this.request<TranscriptionJob>(`/recordings/${recordingId}/transcribe/status`),

    getResult: (recordingId: string) =>
      this.request<Transcript>(`/recordings/${recordingId}/transcribe/result`),

    retry: (recordingId: string) =>
      this.request(`/recordings/${recordingId}/transcribe/retry`, { method: 'POST' }),

    cancel: (recordingId: string) =>
      this.request(`/recordings/${recordingId}/transcribe/cancel`, { method: 'POST' }),
  };

  // Diarization
  diarization = {
    start: (recordingId: string, config?: DiarizationConfig) =>
      this.request<DiarizationJob>(`/recordings/${recordingId}/diarize`, {
        method: 'POST',
        body: JSON.stringify(config || {}),
      }),

    getStatus: (recordingId: string) =>
      this.request<DiarizationJob>(`/recordings/${recordingId}/diarize/status`),

    getResult: (recordingId: string) =>
      this.request<DiarizationResult>(`/recordings/${recordingId}/diarize/result`),

    updateSpeakers: (recordingId: string, speakers: Speaker[]) =>
      this.request<DiarizationResult>(`/recordings/${recordingId}/diarize/speakers`, {
        method: 'PUT',
        body: JSON.stringify({ speakers }),
      }),
  };

  // Summaries
  summaries = {
    generate: (recordingId: string, config: SummaryConfig) =>
      this.request<SummaryJob>(`/recordings/${recordingId}/summaries`, {
        method: 'POST',
        body: JSON.stringify(config),
      }),

    getStatus: (recordingId: string, summaryId: string) =>
      this.request<SummaryJob>(`/recordings/${recordingId}/summaries/${summaryId}/status`),

    getResult: (recordingId: string, summaryId: string) =>
      this.request<Summary>(`/recordings/${recordingId}/summaries/${summaryId}`),

    list: (recordingId: string) =>
      this.request<Summary[]>(`/recordings/${recordingId}/summaries`),

    delete: (recordingId: string, summaryId: string) =>
      this.request(`/recordings/${recordingId}/summaries/${summaryId}`, { method: 'DELETE' }),

    regenerate: (recordingId: string, summaryId: string, config: SummaryConfig) =>
      this.request<SummaryJob>(`/recordings/${recordingId}/summaries/${summaryId}/regenerate`, {
        method: 'POST',
        body: JSON.stringify(config),
      }),
  };

  // Embeddings
  embeddings = {
    generate: (recordingId: string, config?: EmbeddingConfig) =>
      this.request<EmbeddingJob>(`/recordings/${recordingId}/embeddings`, {
        method: 'POST',
        body: JSON.stringify(config || {}),
      }),

    getStatus: (recordingId: string) =>
      this.request<EmbeddingJob>(`/recordings/${recordingId}/embeddings/status`),

    search: (query: string, filters?: SearchFilters) =>
      this.request<SearchResponse>('/search', {
        method: 'POST',
        body: JSON.stringify({ query, ...filters }),
      }),
  };

  // Chat/RAG
  chat = {
    sendMessage: (recordingId: string, message: string, conversationId?: string) =>
      this.request<ChatResponse>(`/recordings/${recordingId}/chat`, {
        method: 'POST',
        body: JSON.stringify({ message, conversation_id: conversationId }),
      }),

    getHistory: (recordingId: string, conversationId?: string) =>
      this.request<ChatMessage[]>(`/recordings/${recordingId}/chat/history`, {
        params: { conversation_id: conversationId },
      }),

    getConversations: (recordingId: string) =>
      this.request<Conversation[]>(`/recordings/${recordingId}/chat/conversations`),

    createConversation: (recordingId: string, title: string) =>
      this.request<Conversation>(`/recordings/${recordingId}/chat/conversations`, {
        method: 'POST',
        body: JSON.stringify({ title }),
      }),
  };

  // Search (Semantic)
  search = {
    query: (request: SearchRequest) =>
      this.request<SearchResponse>('/search', {
        method: 'POST',
        body: JSON.stringify(request),
      }),

    simple: (q: string, limit?: number) =>
      this.request<SearchResponse>('/search/simple', {
        params: { q, limit },
      }),
  };

  // Analytics
  analytics = {
    getOverview: (params?: { start_date?: string; end_date?: string }) =>
      this.request<AnalyticsOverview>('/analytics/overview', { params }),

    getMeetingStats: (params?: { start_date?: string; end_date?: string }) =>
      this.request<MeetingStats>('/analytics/meetings', { params }),

    getSpeakerStats: (params?: { start_date?: string; end_date?: string }) =>
      this.request<SpeakerStats>('/analytics/speakers', { params }),

    getTopicTrends: (params?: { start_date?: string; end_date?: string; limit?: number }) =>
      this.request<TopicTrends>('/analytics/topics', { params }),

    getProductivity: (params?: { start_date?: string; end_date?: string }) =>
      this.request<ProductivityMetrics>('/analytics/productivity', { params }),
  };

  // Users (Admin)
  users = {
    list: (params?: { page?: number; limit?: number; role?: string }) =>
      this.request<PaginatedResponse<User>>('/users', { params }),

    get: (id: string) =>
      this.request<User>(`/users/${id}`),

    update: (id: string, data: Partial<User>) =>
      this.request<User>(`/users/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data),
      }),

    delete: (id: string) =>
      this.request(`/users/${id}`, { method: 'DELETE' }),

    changeRole: (id: string, role: 'admin' | 'user') =>
      this.request<User>(`/users/${id}/role`, {
        method: 'PUT',
        body: JSON.stringify({ role }),
      }),
  };

  // API Keys
  apiKeys = {
    list: () =>
      this.request<ApiKey[]>('/api-keys'),

    create: (data: { name: string; expires_in_days?: number; scopes?: string[] }) =>
      this.request<ApiKey>(`/api-keys`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),

    revoke: (id: string) =>
      this.request(`/api-keys/${id}`, { method: 'DELETE' }),

    getUsage: (id: string) =>
      this.request<ApiKeyUsage>(`/api-keys/${id}/usage`),
  };

  // Health check
  health = {
    check: () =>
      this.request<{ status: string; timestamp: string; version: string }>('/health', {
        // Don't require auth for health check
        headers: { 'Authorization': '' },
      }),

    ready: () =>
      this.request<{ ready: boolean; checks: Record<string, boolean> }>('/health/ready', {
        headers: { 'Authorization': '' },
      }),
  };
}

// Type definitions matching the Rust backend
export interface User {
  id: string;
  email: string;
  full_name: string;
  role: 'admin' | 'user';
  is_active: boolean;
  created_at: string;
  updated_at: string;
  last_login_at?: string;
}

export interface Recording {
  id: string;
  user_id: string;
  meeting_name: string;
  status: 'recording' | 'paused' | 'stopped' | 'processing' | 'completed' | 'failed';
  duration_seconds: number;
  file_path?: string;
  file_size_bytes?: number;
  mime_type?: string;
  device_config?: DeviceConfig;
  created_at: string;
  updated_at: string;
  completed_at?: string;
  transcripts?: Transcript[];
  diarization?: DiarizationResult;
  summaries?: Summary[];
}

export interface DeviceConfig {
  mic_device_name?: string;
  system_device_name?: string;
  sample_rate: number;
  channels: number;
}

export interface Transcript {
  id: string;
  recording_id: string;
  text: string;
  language: string;
  confidence: number;
  provider: string;
  model: string;
  duration_seconds: number;
  created_at: string;
  segments?: TranscriptSegment[];
}

export interface TranscriptSegment {
  id: string;
  transcript_id: string;
  text: string;
  start_time: number;
  end_time: number;
  speaker_id?: string;
  confidence: number;
}

export interface DiarizationResult {
  id: string;
  recording_id: string;
  num_speakers: number;
  speakers: Speaker[];
  created_at: string;
}

export interface Speaker {
  id: string;
  label: string;
  display_name?: string;
  duration_seconds: number;
  segment_count: number;
  color?: string;
}

export interface Summary {
  id: string;
  recording_id: string;
  summary_type: 'brief' | 'detailed' | 'action_items' | 'decisions' | 'topics' | 'custom';
  content: string;
  provider: string;
  model: string;
  tokens_used: number;
  created_at: string;
}

export interface TranscriptionConfig {
  provider?: string;
  model?: string;
  language?: string;
  options?: Record<string, any>;
}

export interface DiarizationConfig {
  min_speakers?: number;
  max_speakers?: number;
  provider?: string;
}

export interface SummaryConfig {
  summary_type: 'brief' | 'detailed' | 'action_items' | 'decisions' | 'topics' | 'custom';
  provider?: string;
  model?: string;
  custom_prompt?: string;
  options?: Record<string, any>;
}

export interface EmbeddingConfig {
  provider?: string;
  model?: string;
  chunk_size?: number;
  chunk_overlap?: number;
}

export interface SearchRequest {
  query: string;
  filters?: SearchFilters;
  limit?: number;
  offset?: number;
}

export interface SearchFilters {
  recording_ids?: string[];
  date_from?: string;
  date_to?: string;
  speakers?: string[];
  summary_types?: string[];
  min_relevance?: number;
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  query_time_ms: number;
}

export interface SearchResult {
  recording_id: string;
  meeting_name: string;
  relevance_score: number;
  matched_content: string;
  context_before?: string;
  context_after?: string;
  timestamp?: number;
  speaker?: string;
  source_type: 'transcript' | 'summary' | 'embedding';
}

export interface SearchResultItem {
  recording_id: string;
  meeting_name: string;
  relevance_score: number;
  matched_content: string;
  timestamp?: number;
  speaker?: string;
}

export interface ChatMessage {
  id: string;
  conversation_id: string;
  role: 'user' | 'assistant';
  content: string;
  citations?: Citation[];
  created_at: string;
}

export interface ChatResponse {
  message: ChatMessage;
  conversation_id: string;
}

export interface Conversation {
  id: string;
  recording_id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

export interface Citation {
  recording_id: string;
  transcript_segment_id?: string;
  summary_id?: string;
  text: string;
  timestamp?: number;
  speaker?: string;
  relevance_score: number;
}

export interface EmbeddingJob {
  id: string;
  recording_id: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  progress: number;
  total_chunks: number;
  processed_chunks: number;
  error?: string;
  created_at: string;
  completed_at?: string;
}

export interface TranscriptionJob {
  id: string;
  recording_id: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  progress: number;
  provider: string;
  model: string;
  error?: string;
  created_at: string;
  completed_at?: string;
}

export interface DiarizationJob {
  id: string;
  recording_id: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  progress: number;
  provider: string;
  error?: string;
  created_at: string;
  completed_at?: string;
}

export interface SummaryJob {
  id: string;
  recording_id: string;
  summary_type: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  progress: number;
  provider: string;
  model: string;
  error?: string;
  created_at: string;
  completed_at?: string;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  total_pages: number;
}

export interface MeetingAnalytics {
  recording_id: string;
  total_duration_seconds: number;
  speaker_count: number;
  word_count: number;
  speaking_time_by_speaker: Record<string, number>;
  topics: TopicInfo[];
  sentiment?: SentimentInfo;
  action_items: ActionItem[];
}

export interface TopicInfo {
  topic: string;
  relevance: number;
  keywords: string[];
  segments: TranscriptSegment[];
}

export interface SentimentInfo {
  overall: 'positive' | 'neutral' | 'negative';
  score: number;
  by_speaker: Record<string, { sentiment: string; score: number }>;
}

export interface ActionItem {
  id: string;
  description: string;
  assignee?: string;
  due_date?: string;
  status: 'pending' | 'in_progress' | 'completed';
  source_segment_id?: string;
}

export interface AnalyticsOverview {
  total_meetings: number;
  total_duration_hours: number;
  avg_meeting_duration_minutes: number;
  total_participants: number;
  meetings_this_week: number;
  meetings_this_month: number;
}

export interface MeetingStats {
  by_date: Record<string, number>;
  by_duration: Record<string, number>;
  by_participant_count: Record<string, number>;
}

export interface SpeakerStats {
  total_speakers: number;
  top_speakers: Array<{ name: string; total_time_seconds: number; meeting_count: number }>;
  speaker_diversity_index: number;
}

export interface TopicTrends {
  trending_topics: Array<{ topic: string; frequency: number; trend: 'up' | 'down' | 'stable' }>;
  topic_cloud: Array<{ topic: string; weight: number }>;
}

export interface ProductivityMetrics {
  action_items_created: number;
  action_items_completed: number;
  completion_rate: number;
  decisions_made: number;
  avg_action_items_per_meeting: number;
}

export interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  expires_at?: string;
  last_used_at?: string;
  created_at: string;
}

export interface ApiKeyUsage {
  total_requests: number;
  requests_today: number;
  requests_this_month: number;
  last_used_endpoint?: string;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public details?: any
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// Export singleton instance
export const apiClient = new ApiClient();

// Helper hook for React components
export function useApiClient() {
  return apiClient;
}

// Type helper for API responses
export type ApiResponse<T> = 
  | { data: T; error: null }
  | { data: null; error: ApiError };

export async function handleApi<T>(promise: Promise<T>): Promise<ApiResponse<T>> {
  try {
    const data = await promise;
    return { data, error: null };
  } catch (error) {
    if (error instanceof ApiError) {
      return { data: null, error };
    }
    return { data: null, error: new ApiError(0, 'Unknown error', error) };
  }
}