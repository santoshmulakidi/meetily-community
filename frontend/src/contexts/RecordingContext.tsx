/**
 * Web Recording Context
 * 
 * Manages recording state and operations for the web version.
 * Uses BrowserAudioRecorder for capture and API client for backend communication.
 */

'use client';

import { createContext, useContext, useState, useEffect, useCallback, ReactNode, useRef } from 'react';
import { apiClient, Recording, Transcript, DiarizationResult, Summary } from '@/lib/apiClient';
import { useAudioRecorder, AudioRecorderState } from '@/lib/browserAudioRecorder';

export type RecordingStatus = 
  | 'idle'
  | 'recording'
  | 'paused'
  | 'stopping'
  | 'uploading'
  | 'processing'
  | 'completed'
  | 'error';

export interface MeetingData {
  id: string;
  meetingName: string;
  status: RecordingStatus;
  duration: number;
  recording?: Recording;
  transcripts?: Transcript[];
  diarization?: DiarizationResult;
  summaries?: Summary[];
  error?: string;
}

interface RecordingContextType {
  // Current meeting state
  currentMeeting: MeetingData | null;
  meetingTitle: string;
  setMeetingTitle: (title: string) => void;
  
  // Recording state
  recordingState: AudioRecorderState;
  isRecording: boolean;
  isPaused: boolean;
  recordingDuration: number;
  
  // Audio
  audioLevel: number;
  selectedDevice: string | null;
  availableDevices: MediaDeviceInfo[];
  
  // Operations
  initializeRecorder: () => Promise<void>;
  startRecording: () => Promise<void>;
  pauseRecording: () => void;
  resumeRecording: () => void;
  stopRecording: () => Promise<void>;
  selectDevice: (deviceId: string) => void;
  
  // Processing
  processingStatus: 'idle' | 'transcribing' | 'diarizing' | 'summarizing' | 'embedding' | 'completed' | 'error';
  processingProgress: number;
  processingError: string | null;
  
  // Meetings list
  meetings: MeetingData[];
  fetchMeetings: () => Promise<void>;
  deleteMeeting: (id: string) => Promise<void>;
}

const RecordingContext = createContext<RecordingContextType | undefined>(undefined);

export function RecordingProvider({ children }: { children: ReactNode }) {
  const [currentMeeting, setCurrentMeeting] = useState<MeetingData | null>(null);
  const [meetingTitle, setMeetingTitle] = useState('');
  const [recordingDuration, setRecordingDuration] = useState(0);
  const [processingStatus, setProcessingStatus] = useState<RecordingContextType['processingStatus']>('idle');
  const [processingProgress, setProcessingProgress] = useState(0);
  const [processingError, setProcessingError] = useState<string | null>(null);
  const [meetings, setMeetings] = useState<MeetingData[]>([]);
  const durationIntervalRef = useRef<NodeJS.Timeout>();

  // Audio recorder hook
  const {
    state: recorderState,
    devices,
    selectedDevice,
    audioLevel,
    error: recorderError,
    initialize,
    startRecording: startAudioRecording,
    pauseRecording: pauseAudioRecording,
    resumeRecording: resumeAudioRecording,
    stopRecording: stopAudioRecording,
    selectDevice,
  } = useAudioRecorder({
    mimeType: 'audio/webm;codecs=opus',
    sampleRate: 16000,
    channelCount: 1,
  });

  // Initialize recorder on mount
  useEffect(() => {
    initialize().catch(err => console.error('Failed to initialize recorder:', err));
  }, [initialize]);

  // Duration timer
  useEffect(() => {
    if (recorderState === 'recording') {
      durationIntervalRef.current = setInterval(() => {
        setRecordingDuration(prev => prev + 1);
      }, 1000);
    } else {
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
      }
    }
    return () => {
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
      }
    };
  }, [recorderState]);

  // Upload audio chunks during recording
  const uploadAudioChunk = useCallback(async (blob: Blob, chunkIndex: number, totalChunks: number) => {
    if (!currentMeeting?.id) return;
    
    try {
      await apiClient.recordings.uploadAudio(currentMeeting.id, blob, chunkIndex, totalChunks);
    } catch (error) {
      console.error('Failed to upload audio chunk:', error);
      // Don't throw - we'll retry on finalize
    }
  }, [currentMeeting]);

  // Start recording
  const startRecording = useCallback(async () => {
    if (!meetingTitle.trim()) {
      throw new Error('Meeting title is required');
    }

    setProcessingError(null);
    setRecordingDuration(0);

    try {
      // Create recording on server
      const recording = await apiClient.recordings.create({
        meeting_name: meetingTitle,
      });

      const newMeeting: MeetingData = {
        id: recording.id,
        meetingName: recording.meeting_name,
        status: 'recording',
        duration: 0,
        recording,
      };

      setCurrentMeeting(newMeeting);
      
      // Start browser recording with upload callback
      await startAudioRecording(uploadAudioChunk);
    } catch (error) {
      setProcessingError(error instanceof Error ? error.message : 'Failed to start recording');
      throw error;
    }
  }, [meetingTitle, startAudioRecording, uploadAudioChunk]);

  // Pause recording
  const pauseRecording = useCallback(() => {
    pauseAudioRecording();
    if (currentMeeting) {
      setCurrentMeeting(prev => prev ? { ...prev, status: 'paused' } : null);
    }
  }, [pauseAudioRecording, currentMeeting]);

  // Resume recording
  const resumeRecording = useCallback(() => {
    resumeAudioRecording();
    if (currentMeeting) {
      setCurrentMeeting(prev => prev ? { ...prev, status: 'recording' } : null);
    }
  }, [resumeAudioRecording, currentMeeting]);

  // Stop recording
  const stopRecording = useCallback(async () => {
    if (!currentMeeting) return;

    setCurrentMeeting(prev => prev ? { ...prev, status: 'stopping' } : null);

    try {
      // Stop browser recording and get final blob
      const audioBlob = await stopAudioRecording();

      // Finalize audio on server
      await apiClient.recordings.finalizeAudio(currentMeeting.id);

      setCurrentMeeting(prev => prev ? { ...prev, status: 'processing', duration: recordingDuration } : null);
      setProcessingStatus('transcribing');
      setProcessingProgress(0);

      // Start transcription
      await apiClient.transcription.start(currentMeeting.id);

      // Poll for completion
      pollTranscription(currentMeeting.id);
    } catch (error) {
      setProcessingError(error instanceof Error ? error.message : 'Failed to stop recording');
      setCurrentMeeting(prev => prev ? { 
        ...prev, 
        status: 'error',
        error: error instanceof Error ? error.message : 'Unknown error'
      } : null);
    }
  }, [currentMeeting, recordingDuration, stopAudioRecording]);

  // Poll transcription status
  const pollTranscription = useCallback(async (recordingId: string) => {
    const checkStatus = async () => {
      try {
        const job = await apiClient.transcription.getStatus(recordingId);
        
        if (job.status === 'processing') {
          setProcessingProgress(job.progress);
          setTimeout(checkStatus, 2000);
        } else if (job.status === 'completed') {
          setProcessingStatus('diarizing');
          setProcessingProgress(0);
          await pollDiarization(recordingId);
        } else if (job.status === 'failed') {
          throw new Error(job.error || 'Transcription failed');
        }
      } catch (error) {
        setProcessingError(error instanceof Error ? error.message : 'Transcription failed');
        setCurrentMeeting(prev => prev ? { ...prev, status: 'error' } : null);
      }
    };
    checkStatus();
  }, []);

  // Poll diarization status
  const pollDiarization = useCallback(async (recordingId: string) => {
    const checkStatus = async () => {
      try {
        const job = await apiClient.diarization.getStatus(recordingId);
        
        if (job.status === 'processing') {
          setProcessingProgress(job.progress);
          setTimeout(checkStatus, 2000);
        } else if (job.status === 'completed') {
          setProcessingStatus('summarizing');
          setProcessingProgress(0);
          await pollSummary(recordingId);
        } else if (job.status === 'failed') {
          throw new Error(job.error || 'Diarization failed');
        }
      } catch (error) {
        setProcessingError(error instanceof Error ? error.message : 'Diarization failed');
        setCurrentMeeting(prev => prev ? { ...prev, status: 'error' } : null);
      }
    };
    checkStatus();
  }, []);

  // Poll summary status
  const pollSummary = useCallback(async (recordingId: string) => {
    const checkStatus = async () => {
      try {
        const summaries = await apiClient.summaries.list(recordingId);
        const pending = summaries.find(s => 
          ['pending', 'processing'].includes(s.id) // Would need actual job status
        );
        
        if (pending) {
          setTimeout(checkStatus, 3000);
        } else {
          setProcessingStatus('embedding');
          setProcessingProgress(0);
          await pollEmbeddings(recordingId);
        }
      } catch (error) {
        // Summaries might not be enabled, continue to embeddings
        setProcessingStatus('embedding');
        setProcessingProgress(0);
        await pollEmbeddings(recordingId);
      }
    };
    checkStatus();
  }, []);

  // Poll embeddings status
  const pollEmbeddings = useCallback(async (recordingId: string) => {
    const checkStatus = async () => {
      try {
        const job = await apiClient.embeddings.getStatus(recordingId);
        
        if (job.status === 'processing') {
          setProcessingProgress(job.progress);
          setTimeout(checkStatus, 3000);
        } else if (job.status === 'completed') {
          // All done!
          await finalizeMeeting(recordingId);
        } else if (job.status === 'failed') {
          // Embeddings failed but meeting is still usable
          await finalizeMeeting(recordingId);
        }
      } catch (error) {
        // Embeddings might not be enabled
        await finalizeMeeting(recordingId);
      }
    };
    checkStatus();
  }, []);

  // Finalize meeting - fetch all results
  const finalizeMeeting = useCallback(async (recordingId: string) => {
    try {
      const [recording, transcripts, diarization, summaries] = await Promise.all([
        apiClient.recordings.get(recordingId),
        apiClient.recordings.getTranscripts(recordingId),
        apiClient.recordings.getDiarization(recordingId).catch(() => null),
        apiClient.recordings.getSummaries(recordingId).catch(() => []),
      ]);

      const completedMeeting: MeetingData = {
        id: recordingId,
        meetingName: recording.meeting_name,
        status: 'completed',
        duration: recordingDuration,
        recording,
        transcripts,
        diarization: diarization || undefined,
        summaries,
      };

      setCurrentMeeting(completedMeeting);
      setProcessingStatus('completed');
      setProcessingProgress(100);
      
      // Refresh meetings list
      await fetchMeetings();
    } catch (error) {
      setProcessingError(error instanceof Error ? error.message : 'Failed to finalize meeting');
      setCurrentMeeting(prev => prev ? { ...prev, status: 'error' } : null);
    }
  }, [recordingDuration]);

  // Fetch all meetings
  const fetchMeetings = useCallback(async () => {
    try {
      const response = await apiClient.recordings.list({ limit: 50 });
      const meetingsData: MeetingData[] = response.data.map(r => ({
        id: r.id,
        meetingName: r.meeting_name,
        status: r.status === 'completed' ? 'completed' : 
                r.status === 'failed' ? 'error' : 'processing',
        duration: r.duration_seconds,
        recording: r,
      }));
      setMeetings(meetingsData);
    } catch (error) {
      console.error('Failed to fetch meetings:', error);
    }
  }, []);

  // Delete meeting
  const deleteMeeting = useCallback(async (id: string) => {
    await apiClient.recordings.delete(id);
    setMeetings(prev => prev.filter(m => m.id !== id));
    if (currentMeeting?.id === id) {
      setCurrentMeeting(null);
    }
  }, [currentMeeting]);

  return (
    <RecordingContext.Provider value={{
      currentMeeting,
      meetingTitle,
      setMeetingTitle,
      recordingState: recorderState,
      isRecording: recorderState === 'recording',
      isPaused: recorderState === 'paused',
      recordingDuration,
      audioLevel,
      selectedDevice,
      availableDevices: devices,
      initializeRecorder: initialize,
      startRecording,
      pauseRecording,
      resumeRecording,
      stopRecording,
      selectDevice,
      processingStatus,
      processingProgress,
      processingError,
      meetings,
      fetchMeetings,
      deleteMeeting,
    }}>
      {children}
    </RecordingContext.Provider>
  );
}

export function useRecording() {
  const context = useContext(RecordingContext);
  if (context === undefined) {
    throw new Error('useRecording must be used within a RecordingProvider');
  }
  return context;
}