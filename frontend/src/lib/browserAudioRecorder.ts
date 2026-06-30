/**
 * Browser Audio Recorder Service
 * 
 * Uses MediaRecorder API for web-based audio recording.
 * Replaces the Tauri cpal/cidre desktop audio capture.
 */

export interface AudioRecorderConfig {
  mimeType?: string;
  audioBitsPerSecond?: number;
  sampleRate?: number;
  channelCount?: number;
  echoCancellation?: boolean;
  noiseSuppression?: boolean;
  autoGainControl?: boolean;
}

export interface AudioChunk {
  blob: Blob;
  timestamp: number;
  chunkIndex: number;
  isFinal: boolean;
}

export type AudioRecorderState = 'inactive' | 'recording' | 'paused';

export interface AudioRecorderCallbacks {
  onDataAvailable?: (chunk: AudioChunk) => void;
  onStateChange?: (state: AudioRecorderState) => void;
  onError?: (error: Error) => void;
  onWarning?: (warning: string) => void;
}

const DEFAULT_CONFIG: Required<AudioRecorderConfig> = {
  mimeType: 'audio/webm;codecs=opus',
  audioBitsPerSecond: 128000,
  sampleRate: 16000,
  channelCount: 1,
  echoCancellation: true,
  noiseSuppression: true,
  autoGainControl: true,
};

export class BrowserAudioRecorder {
  private mediaRecorder: MediaRecorder | null = null;
  private mediaStream: MediaStream | null = null;
  private config: Required<AudioRecorderConfig>;
  private callbacks: AudioRecorderCallbacks;
  private state: AudioRecorderState = 'inactive';
  private chunkIndex = 0;
  private startTime = 0;
  private recordingChunks: Blob[] = [];
  private uploadQueue: Blob[] = [];
  private isUploading = false;
  private uploadCallback?: (blob: Blob, chunkIndex: number, totalChunks: number) => Promise<void>;

  constructor(config: AudioRecorderConfig = {}, callbacks: AudioRecorderCallbacks = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.callbacks = callbacks;
  }

  /**
   * Initialize the recorder with microphone access
   */
  async initialize(deviceId?: string): Promise<void> {
    try {
      const constraints: MediaStreamConstraints = {
        audio: {
          deviceId: deviceId ? { exact: deviceId } : undefined,
          echoCancellation: this.config.echoCancellation,
          noiseSuppression: this.config.noiseSuppression,
          autoGainControl: this.config.autoGainControl,
          sampleRate: this.config.sampleRate,
          channelCount: this.config.channelCount,
        },
      };

      this.mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
      
      // Check if the mime type is supported
      if (!MediaRecorder.isTypeSupported(this.config.mimeType)) {
        // Fallback to supported types
        const fallbacks = [
          'audio/webm;codecs=opus',
          'audio/webm',
          'audio/ogg;codecs=opus',
          'audio/ogg',
          'audio/mp4',
          'audio/wav',
        ];
        
        for (const type of fallbacks) {
          if (MediaRecorder.isTypeSupported(type)) {
            this.config.mimeType = type;
            break;
          }
        }
      }

      this.mediaRecorder = new MediaRecorder(this.mediaStream, {
        mimeType: this.config.mimeType,
        audioBitsPerSecond: this.config.audioBitsPerSecond,
      });

      this.mediaRecorder.ondataavailable = this.handleDataAvailable.bind(this);
      this.mediaRecorder.onstop = this.handleStop.bind(this);
      this.mediaRecorder.onerror = this.handleError.bind(this);
      this.mediaRecorder.onwarning = this.handleWarning.bind(this);

      this.setState('inactive');
    } catch (error) {
      this.callbacks.onError?.(error as Error);
      throw error;
    }
  }

  /**
   * Start recording
   */
  async start(uploadCallback?: (blob: Blob, chunkIndex: number, totalChunks: number) => Promise<void>): Promise<void> {
    if (!this.mediaRecorder || this.state === 'recording') {
      return;
    }

    this.uploadCallback = uploadCallback;
    this.chunkIndex = 0;
    this.startTime = Date.now();
    this.recordingChunks = [];
    this.uploadQueue = [];

    try {
      // Start with 100ms timeslices for near real-time processing
      this.mediaRecorder.start(100);
      this.setState('recording');
    } catch (error) {
      this.callbacks.onError?.(error as Error);
      throw error;
    }
  }

  /**
   * Pause recording
   */
  pause(): void {
    if (this.mediaRecorder && this.state === 'recording') {
      this.mediaRecorder.pause();
      this.setState('paused');
    }
  }

  /**
   * Resume recording
   */
  resume(): void {
    if (this.mediaRecorder && this.state === 'paused') {
      this.mediaRecorder.resume();
      this.setState('recording');
    }
  }

  /**
   * Stop recording and finalize
   */
  async stop(): Promise<Blob> {
    if (!this.mediaRecorder || this.state === 'inactive') {
      throw new Error('Recorder is not active');
    }

    return new Promise((resolve, reject) => {
      const finalize = () => {
        // Combine all chunks into a single blob
        const finalBlob = new Blob(this.recordingChunks, { type: this.config.mimeType });
        this.setState('inactive');
        this.cleanup();
        resolve(finalBlob);
      };

      // Set a one-time handler for the final data
      this.mediaRecorder!.onstop = () => {
        this.handleStop();
        finalize();
      };

      this.mediaRecorder!.stop();
    });
  }

  /**
   * Get current recording state
   */
  getState(): AudioRecorderState {
    return this.state;
  }

  /**
   * Get available audio input devices
   */
  static async getAudioDevices(): Promise<MediaDeviceInfo[]> {
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices.filter(device => device.kind === 'audioinput');
  }

  /**
   * Get current audio stream (for visualization)
   */
  getMediaStream(): MediaStream | null {
    return this.mediaStream;
  }

  /**
   * Create audio context for real-time visualization
   */
  createAudioContext(): AudioContext | null {
    if (!this.mediaStream) return null;
    
    const audioContext = new AudioContext();
    const source = audioContext.createMediaStreamSource(this.mediaStream);
    const analyser = audioContext.createAnalyser();
    analyser.fftSize = 256;
    source.connect(analyser);
    
    return audioContext;
  }

  /**
   * Get audio level for visualization (0-1)
   */
  getAudioLevel(analyser: AnalyserNode): number {
    const dataArray = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(dataArray);
    
    const sum = dataArray.reduce((acc, val) => acc + val, 0);
    const average = sum / dataArray.length;
    
    // Normalize to 0-1 range (adjust sensitivity as needed)
    return Math.min(average / 128, 1);
  }

  /**
   * Set upload callback for streaming chunks to server
   */
  setUploadCallback(callback: (blob: Blob, chunkIndex: number, totalChunks: number) => Promise<void>): void {
    this.uploadCallback = callback;
  }

  private handleDataAvailable(event: BlobEvent): void {
    if (event.data.size === 0) return;

    const chunk: AudioChunk = {
      blob: event.data,
      timestamp: Date.now() - this.startTime,
      chunkIndex: this.chunkIndex++,
      isFinal: false,
    };

    this.recordingChunks.push(event.data);
    this.callbacks.onDataAvailable?.(chunk);

    // Queue for upload if callback is set
    if (this.uploadCallback) {
      this.uploadQueue.push(event.data);
      this.processUploadQueue();
    }
  }

  private async processUploadQueue(): Promise<void> {
    if (this.isUploading || this.uploadQueue.length === 0 || !this.uploadCallback) {
      return;
    }

    this.isUploading = true;

    while (this.uploadQueue.length > 0) {
      const blob = this.uploadQueue.shift()!;
      const index = this.chunkIndex - this.uploadQueue.length - 1;
      
      try {
        await this.uploadCallback(blob, index, this.chunkIndex);
      } catch (error) {
        this.callbacks.onError?.(error as Error);
        // Re-queue on failure
        this.uploadQueue.unshift(blob);
        break;
      }
    }

    this.isUploading = false;
  }

  private handleStop(): void {
    this.setState('inactive');
    this.cleanup();
  }

  private handleError(event: Event): void {
    const error = new Error('MediaRecorder error');
    this.callbacks.onError?.(error);
    this.setState('inactive');
  }

  private handleWarning(event: Event): void {
    const warning = 'MediaRecorder warning';
    this.callbacks.onWarning?.(warning);
  }

  private setState(state: AudioRecorderState): void {
    this.state = state;
    this.callbacks.onStateChange?.(state);
  }

  private cleanup(): void {
    if (this.mediaStream) {
      this.mediaStream.getTracks().forEach(track => track.stop());
      this.mediaStream = null;
    }
    this.mediaRecorder = null;
  }

  /**
   * Clean up all resources
   */
  destroy(): void {
    if (this.state === 'recording' || this.state === 'paused') {
      this.mediaRecorder?.stop();
    }
    this.cleanup();
    this.callbacks = {};
  }
}

// React hook for using the audio recorder
import { useState, useEffect, useCallback, useRef } from 'react';

export interface UseAudioRecorderReturn {
  recorder: BrowserAudioRecorder | null;
  state: AudioRecorderState;
  devices: MediaDeviceInfo[];
  selectedDevice: string | null;
  error: Error | null;
  initialize: (deviceId?: string) => Promise<void>;
  startRecording: (uploadCallback?: (blob: Blob, chunkIndex: number, totalChunks: number) => Promise<void>) => Promise<void>;
  pauseRecording: () => void;
  resumeRecording: () => void;
  stopRecording: () => Promise<Blob>;
  selectDevice: (deviceId: string) => void;
  audioLevel: number;
}

export function useAudioRecorder(config?: AudioRecorderConfig): UseAudioRecorderReturn {
  const [recorder, setRecorder] = useState<BrowserAudioRecorder | null>(null);
  const [state, setState] = useState<AudioRecorderState>('inactive');
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const animationRef = useRef<number>();

  // Load devices on mount
  useEffect(() => {
    BrowserAudioRecorder.getAudioDevices().then(setDevices);
  }, []);

  // Initialize recorder
  const initialize = useCallback(async (deviceId?: string) => {
    try {
      setError(null);
      const newRecorder = new BrowserAudioRecorder(config, {
        onStateChange: setState,
        onError: setError,
        onWarning: (w) => console.warn('Audio warning:', w),
      });
      
      await newRecorder.initialize(deviceId || selectedDevice || undefined);
      setRecorder(newRecorder);

      // Set up audio level monitoring
      const audioContext = newRecorder.createAudioContext();
      if (audioContext) {
        const source = audioContext.createMediaStreamSource(newRecorder.getMediaStream()!);
        const analyser = audioContext.createAnalyser();
        analyser.fftSize = 256;
        source.connect(analyser);
        analyserRef.current = analyser;

        const updateLevel = () => {
          if (analyserRef.current && state === 'recording') {
            const level = newRecorder.getAudioLevel(analyserRef.current);
            setAudioLevel(level);
          }
          animationRef.current = requestAnimationFrame(updateLevel);
        };
        updateLevel();
      }
    } catch (err) {
      setError(err as Error);
    }
  }, [config, selectedDevice, state]);

  // Start recording
  const startRecording = useCallback(async (
    uploadCallback?: (blob: Blob, chunkIndex: number, totalChunks: number) => Promise<void>
  ) => {
    if (!recorder) {
      await initialize();
    }
    if (recorder) {
      await recorder.start(uploadCallback);
    }
  }, [recorder, initialize]);

  // Pause recording
  const pauseRecording = useCallback(() => {
    recorder?.pause();
  }, [recorder]);

  // Resume recording
  const resumeRecording = useCallback(() => {
    recorder?.resume();
  }, [recorder]);

  // Stop recording
  const stopRecording = useCallback(async (): Promise<Blob> => {
    if (!recorder) throw new Error('Recorder not initialized');
    
    // Clean up animation
    if (animationRef.current) {
      cancelAnimationFrame(animationRef.current);
    }
    
    return recorder.stop();
  }, [recorder]);

  // Select device
  const selectDevice = useCallback((deviceId: string) => {
    setSelectedDevice(deviceId);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
      recorder?.destroy();
    };
  }, [recorder]);

  return {
    recorder,
    state,
    devices,
    selectedDevice,
    error,
    initialize,
    startRecording,
    pauseRecording,
    resumeRecording,
    stopRecording,
    selectDevice,
    audioLevel,
  };
}