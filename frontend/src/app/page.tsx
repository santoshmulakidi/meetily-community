/**
 * Meeting Dashboard - Web Version
 * Simple version that shows meetings and allows recording
 */

'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useRecording } from '@/contexts/RecordingContext';
import { useAuth } from '@/contexts/AuthContext';
import { useRouter } from 'next/navigation';
import { 
  Mic, 
  List, 
  Settings, 
  LogOut, 
  Plus,
  Trash2,
  Clock
} from 'lucide-react';

export default function MeetingDashboard() {
  const { user, logout, isLoading } = useAuth();
  const { 
    currentMeeting,
    meetingTitle,
    setMeetingTitle,
    startRecording,
    pauseRecording,
    resumeRecording,
    stopRecording,
    selectDevice,
    recordingState,
    isRecording,
    isPaused,
    recordingDuration,
    audioLevel,
    selectedDevice,
    availableDevices,
    meetings,
    fetchMeetings,
    deleteMeeting,
    initializeRecorder,
  } = useRecording();
  const router = useRouter();

  // Initialize on mount
  useEffect(() => {
    if (!user) return;
    initializeRecorder().catch(console.error);
    fetchMeetings();
  }, [initializeRecorder, fetchMeetings, user]);

  useEffect(() => {
    if (!isLoading && !user) {
      router.replace('/login');
    }
  }, [isLoading, router, user]);

  const handleNewMeeting = () => {
    setMeetingTitle('');
    // In a real app, this might open a modal or focus the input
  };

  const handleLogout = async () => {
    await logout();
    router.push('/login');
  };

  if (isLoading || !user) {
    return (
      <div className="flex h-screen items-center justify-center bg-gray-50 text-gray-600">
        Loading...
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar */}
      <aside className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="flex items-center justify-between p-4 border-b border-gray-200">
          <div className="flex items-center space-x-3">
            <Mic className="h-6 w-6 text-indigo-600" />
            <h1 className="text-xl font-semibold text-gray-900">Meetily</h1>
          </div>
          <button onClick={handleLogout} className="text-gray-400 hover:text-gray-600">
            <LogOut className="h-5 w-5" />
          </button>
        </div>

        <nav className="flex-1 overflow-y-auto">
          <ul className="space-y-1 px-3 py-4">
            <li>
              <button
                onClick={handleNewMeeting}
                className="w-full flex items-center px-3 py-2 text-left text-sm font-medium rounded-md hover:bg-gray-100"
              >
                <Plus className="mr-3 h-4 w-4" />
                New Meeting
              </button>
            </li>
            <li>
              <button
                className="w-full flex items-center px-3 py-2 text-left text-sm font-medium rounded-md hover:bg-gray-100"
              >
                <List className="mr-3 h-4 w-4" />
                Meeting History
              </button>
            </li>
            <li>
              <button
                className="w-full flex items-center px-3-py-2 text-left text-sm font-medium rounded-md hover:bg-gray-100"
              >
                <Settings className="mr-3 h-4 w-4" />
                Settings
              </button>
            </li>
          </ul>
        </nav>
      </aside>

      {/* Main Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <header className="flex items-center justify-between px-6 py-4 border-b border-gray-200 bg-white">
          <div className="flex items-center space-x-4">
            <h2 className="text-2xl font-bold text-gray-900">
              {currentMeeting?.meetingName || meetingTitle || 'New Meeting'}
            </h2>
            {currentMeeting && (
              <div className="flex items-center space-x-2 text-sm text-gray-500">
                <Clock className="h-4 w-4" />
                <span>{Math.floor(recordingDuration / 60)}:{String(recordingDuration % 60).padStart(2, '0')}</span>
              </div>
            )}
          </div>
          <div className="flex items-center space-x-3">
            <span className="text-sm text-gray-600">{user?.email?.split('@')[0] ?? 'User'}</span>
            <div className="h-8 w-8 bg-gray-200 rounded-full flex items-center justify-center">
              {user?.email?.charAt(0)?.toUpperCase() ?? 'U'}
            </div>
          </div>
        </header>

        {/* Meeting Content */}
        <div className="flex-1 overflow-hidden">
          {/* Recording Controls */}
          <div className="flex items-center justify-center py-8">
            <div className="bg-white rounded-xl shadow-lg p-6 w-full max-w-md">
              <div className="space-y-6">
                {/* Meeting Title Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Meeting Title
                  </label>
                  <input
                    type="text"
                    value={meetingTitle}
                    onChange={(e) => setMeetingTitle(e.target.value)}
                    placeholder="Enter meeting title (e.g., Team Standup)"
                    className="w-full px-4 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  />
                </div>

                {/* Audio Device Selector */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Microphone
                  </label>
                  <select
                    value={selectedDevice || ''}
                    onChange={(e) => selectDevice(e.target.value)}
                    className="w-full px-4 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  >
                    <option value="">Default Device</option>
                    {availableDevices.map(device => (
                      <option key={device.deviceId} value={device.deviceId}>
                        {device.label}
                      </option>
                    ))}
                  </select>
                </div>

                {/* Recording Controls */}
                <div className="flex items-center justify-center space-x-4">
                  {!isRecording && (
                    <button
                      onClick={async () => {
                        if (!meetingTitle.trim()) {
                          alert('Please enter a meeting title');
                          return;
                        }
                        try {
                          await startRecording();
                        } catch (err) {
                          alert('Failed to start recording: ' + (err instanceof Error ? err.message : 'Unknown error'));
                        }
                      }}
                      className="flex items-center justify-center w-16 h-16 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors duration-200"
                    >
                      <Mic className="h-6 w-6" />
                    </button>
                  )}
                  {isRecording && !isPaused && (
                    <>
                      <button
                        onClick={pauseRecording}
                        className="flex items-center justify-center w-10 h-10 bg-yellow-500 text-white rounded-full hover:bg-yellow-600 transition-colors duration-200"
                      >
                        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9.5 4h3.75a1.5 1.5 0 011.5 1.5v10.5a1.5 1.5 0 01-1.5 1.5h-3.75a1.5 1.5 0 01-1.5-1.5V5.5a1.5 1.5 0 011.5-1.5zM5 10.5a1.5 1.5 0 011.5-1.5h3a1.5 1.5 0 001.5 1.5v6a1.5 1.5 0 00-1.5 1.5H5a1.5 1.5 0 01-1.5-1.5v-6z" /></svg>
                      </button>
                      <button
                        onClick={stopRecording}
                        className="flex items-center justify-center w-10 h-10 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors duration-200"
                      >
                        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" /></svg>
                      </button>
                    </>
                  )}
                  {isPaused && (
                    <>
                      <button
                        onClick={resumeRecording}
                        className="flex items-center justify-center w-10 h-10 bg-green-500 text-white rounded-full hover:bg-green-600 transition-colors duration-200"
                      >
                        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 8v4l3 3-6-6 3 3V8z" /></svg>
                      </button>
                      <button
                        onClick={stopRecording}
                        className="flex items-center justify-center w-10 h-10 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors duration-200"
                      >
                        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" /></svg>
                      </button>
                    </>
                  )}
                </div>

                {/* Audio Level Indicator */}
                <div className="mt-4">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium text-gray-700">Audio Level</span>
                    <span className="text-sm text-gray-500">{Math.floor(audioLevel * 100)}%</span>
                  </div>
                  <div className="w-full bg-gray-200 rounded-full h-2.5">
                    <div className="bg-indigo-500 h-2.5 rounded-full" style={{ width: `${audioLevel * 100}%` }}></div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Recent Meetings List */}
          <div className="flex-1 overflow-y-auto px-6 py-4 bg-gray-50">
            <h3 className="text-lg font-semibold text-gray-900 mb-4">Recent Meetings</h3>
            {meetings.length === 0 ? (
              <div className="text-center py-12 text-gray-500">
                No meetings yet. Start your first recording!
              </div>
            ) : (
              <div className="space-y-4">
                {meetings.map(meeting => (
                  <div key={meeting.id} className="bg-white rounded-xl shadow-sm p-4 hover:shadow-md transition-shadow duration-200">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex-1">
                        <h4 className="font-medium text-gray-900">{meeting.meetingName}</h4>
                        <p className="text-sm text-gray-500">
                          {meeting.recording?.created_at ? new Date(meeting.recording.created_at).toLocaleDateString() : 'Unknown date'} • 
                          {Math.floor(meeting.duration / 60)}:{String(meeting.duration % 60).padStart(2, '0')}
                        </p>
                      </div>
                      <div className="flex items-center space-x-2">
                        {meeting.status === 'recording' && (
                          <span className="px-2 py-0.5 bg-red-100 text-red-800 text-xs rounded-full">Recording</span>
                        )}
                        {meeting.status === 'completed' && (
                          <span className="px-2 py-0.5 bg-green-100 text-green-800 text-xs rounded-full">Completed</span>
                        )}
                        {meeting.status === 'processing' && (
                          <span className="px-2 py-0.5 bg-yellow-100 text-yellow-800 text-xs rounded-full">Processing...</span>
                        )}
                        {meeting.status === 'error' && (
                          <span className="px-2 py-0.5 bg-red-100 text-red-800 text-xs rounded-full">Error</span>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center justify-between text-xs text-gray-400">
                      <span>
                        {/* Would show speaker count, topics, etc. in real app */}
                        {meeting.transcripts?.length ?? 0} transcript segments
                      </span>
                      <button
                        onClick={() => router.push(`/meeting-details?id=${meeting.id}`)}
                        className="text-indigo-600 hover:text-indigo-500 underline"
                      >
                        View Details
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <footer className="flex items-center justify-between px-4 py-3 border-t border-gray-200 bg-white text-sm text-gray-500">
          <span>
            Meetily v1.0 • {user?.email}
          </span>
          <div className="flex items-center space-x-3">
            <span>Privacy-first • Local processing</span>
          </div>
        </footer>
      </main>
    </div>
  );
}
