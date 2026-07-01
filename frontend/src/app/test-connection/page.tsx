/**
 * Test API Connection Page
 * 
 * Drop this file into frontend/src/app/test-connection/page.tsx
 * Then visit http://localhost:3000/test-connection
 * 
 * It will show you:
 * - Whether your Oracle VM backend is reachable
 * - What endpoints are available
 * - Any CORS or authentication errors
 */

'use client';

import { useState } from 'react';
import { apiClient } from '@/lib/apiClient';

export default function TestConnection() {
  const [results, setResults] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const testConnection = async () => {
    setLoading(true);
    const logs: string[] = ['Starting connection test...\n'];

    try {
      // Test 1: Check if we can reach the API
      logs.push('1. Testing basic connectivity...\n');
      
      const response = await fetch(`${apiClient.getBaseUrl()}/health`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (response.ok) {
        logs.push('✓ Backend is reachable!\n');
        logs.push(`  Status: ${response.status}\n`);
        const data = await response.json();
        logs.push(`  Response: ${JSON.stringify(data, null, 2)}\n`);
      } else {
        logs.push(`✗ Backend responded with status ${response.status}\n`);
      }

      // Test 2: Try to get API documentation
      logs.push('\n2. Checking API documentation...\n');
      const docsResponse = await fetch(`${apiClient.getBaseUrl()}/api-docs/openapi.json`);
      if (docsResponse.ok) {
        logs.push('✓ API documentation available\n');
      } else {
        logs.push(`✗ Documentation not found (status: ${docsResponse.status})\n`);
      }

      // Test 3: Try health endpoint
      logs.push('\n3. Testing /api/health endpoint...\n');
      const healthResponse = await fetch(`${apiClient.getBaseUrl()}/health`);
      if (healthResponse.ok) {
        logs.push('✓ Health endpoint working\n');
      } else {
        logs.push(`✗ Health check failed (status: ${healthResponse.status})\n`);
      }

    } catch (error) {
      logs.push(`✗ Error: ${error instanceof Error ? error.message : 'Unknown error'}\n`);
      logs.push('\nPossible issues:\n');
      logs.push('  - Oracle VM backend is not running\n');
      logs.push('  - Firewall blocking port 8082\n');
      logs.push('  - CORS not configured on backend\n');
      logs.push('  - Network connectivity issue\n');
    }

    logs.push('\n---\n');
    logs.push('Current API Base URL:\n');
    logs.push(`  ${apiClient.getBaseUrl()}\n`);
    logs.push('\nTo change the backend URL, edit:\n');
    logs.push('  frontend/src/lib/apiClient.ts\n');
    logs.push('  Line 12: const API_BASE = ...\n');

    setResults(logs);
    setLoading(false);
  };

  return (
    <div className="min-h-screen p-8 bg-gray-900 text-white">
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-bold mb-6">API Connection Test</h1>
        
        <div className="mb-6">
          <button
            onClick={testConnection}
            disabled={loading}
            className="px-6 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 rounded-lg font-semibold transition"
          >
            {loading ? 'Testing...' : 'Test Connection'}
          </button>
        </div>

        {results.length > 0 && (
          <div className="bg-gray-800 rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Test Results:</h2>
            <pre className="whitespace-pre-wrap text-sm font-mono text-gray-300">
              {results.join('')}
            </pre>
          </div>
        )}

        <div className="mt-8 bg-gray-800 rounded-lg p-6">
          <h2 className="text-xl font-semibold mb-4">Expected Backend Configuration:</h2>
          <ul className="list-disc list-inside space-y-2 text-gray-300">
            <li>Backend URL: <code className="bg-gray-700 px-2 py-1 rounded">http://163.192.111.51:8082</code></li>
            <li>API Base Path: <code className="bg-gray-700 px-2 py-1 rounded">/api</code></li>
            <li>Health Check: <code className="bg-gray-700 px-2 py-1 rounded">/api/health</code></li>
            <li>OpenAPI JSON: <code className="bg-gray-700 px-2 py-1 rounded">/api/api-docs/openapi.json</code></li>
          </ul>
        </div>

        <div className="mt-8 bg-yellow-900/30 border border-yellow-700 rounded-lg p-6">
          <h2 className="text-xl font-semibold mb-4 text-yellow-400">Troubleshooting Tips:</h2>
          <ol className="list-decimal list-inside space-y-2 text-gray-300">
            <li>Make sure your Rust backend is running on the Oracle VM</li>
            <li>Check that port 8082 is open in Oracle Cloud firewall</li>
            <li>Verify CORS is enabled in your backend configuration</li>
            <li>Test with: <code className="bg-gray-700 px-2 py-1 rounded">curl http://163.192.111.51:8082/api/health</code></li>
          </ol>
        </div>
      </div>
    </div>
  );
}
