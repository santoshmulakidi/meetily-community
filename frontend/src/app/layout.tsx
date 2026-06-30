/**
 * Main Layout - Web Version
 * 
 * Wraps the app with authentication and recording providers.
 */

import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';
import { AuthProvider } from '@/contexts/AuthContext';
import { RecordingProvider } from '@/contexts/RecordingContext';
import { Toaster } from 'sonner';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Meetily - AI Meeting Assistant',
  description: 'Record, transcribe, and summarize meetings with AI',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className={`${inter.className} antialiased`}>
        <AuthProvider>
          <RecordingProvider>
            {children}
            <Toaster position="top-right" />
          </RecordingProvider>
        </AuthProvider>
      </body>
    </html>
  );
}