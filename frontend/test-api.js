/**
 * Quick test of the API client against your Rust backend
 * 
 * Run this to verify the web frontend can talk to your server:
 * 
 * 1. Start your Rust backend: cd server && cargo run
 * 2. Run this test: node test-api.js
 * 3. Check the output for connection status
 */

const API_BASE_URL = process.env.MEETILY_API_URL || 'http://localhost:8080';

async function testConnection() {
  console.log('🧪 Testing Meetily API Connection\n');
  console.log('Backend URL:', API_BASE_URL);
  console.log('');

  // Test 1: Health check
  console.log('Test 1: Health Check');
  try {
    const health = await fetch(`${API_BASE_URL}/health`);
    const healthData = await health.json();
    console.log('✅ Health check passed:', healthData);
  } catch (error) {
    console.log('❌ Health check failed:', error.message);
    console.log('   → Is your Rust backend running on port 8080?');
    return;
  }

  // Test 2: API version
  console.log('\nTest 2: API Version');
  try {
    const version = await fetch(`${API_BASE_URL}/api/v1/version`);
    const versionData = await version.json();
    console.log('✅ API version:', versionData);
  } catch (error) {
    console.log('❌ Version check failed:', error.message);
  }

  // Test 3: Auth endpoints
  console.log('\nTest 3: Auth Endpoints Availability');
  try {
    const routes = [
      '/api/v1/auth/register',
      '/api/v1/auth/login',
      '/api/v1/auth/me',
    ];
    
    for (const route of routes) {
      const response = await fetch(`${API_BASE_URL}${route}`, { method: 'OPTIONS' });
      console.log(`✅ ${route}: Available (${response.status})`);
    }
  } catch (error) {
    console.log('⚠️  Auth endpoint check failed (might be CORS or not running):', error.message);
  }

  console.log('\n✅ Connection test complete!');
  console.log('\nNext steps:');
  console.log('1. Start your Rust backend: cd server && cargo run');
  console.log('2. Visit http://localhost:3000 in your browser');
  console.log('3. Register a new account or login');
  console.log('4. Start recording a meeting!');
}

testConnection().catch(console.error);