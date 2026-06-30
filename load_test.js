/**
 * Meetily Community+ Load Testing Script
 * 
 * Uses k6 for load testing API endpoints
 * 
 * Installation:
 *   brew install k6  # macOS
 *   sudo apt install k6  # Ubuntu
 * 
 * Usage:
 *   k6 run load_test.js
 *   k6 run -u 100 -d 30s load_test.js  # 100 users for 30 seconds
 *   k6 run --vus 50 --duration 1m load_test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

// ============================================================================
// Custom Metrics
// ============================================================================

const errorRate = new Rate('errors');
const authSuccessRate = new Rate('auth_success');

// ============================================================================
// Configuration
// ============================================================================

export const options = {
  // Scenarios for different load patterns
  scenarios: {
    // Smoke test - quick sanity check
    smoke: {
      executor: 'constant-users',
      vus: 5,
      duration: '30s',
      gracefulStop: '5s',
    },
    
    // Load test - normal operating conditions
    load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '1m', target: 50 },   // Ramp up to 50 users
        { duration: '3m', target: 50 },   // Stay at 50 users
        { duration: '1m', target: 0 },    // Ramp down to 0
      ],
      gracefulRampDown: '30s',
      startTime: '1m',  // Start after smoke test
    },
    
    // Stress test - break the system
    stress: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 100 },  // Ramp to 100
        { duration: '3m', target: 100 },  // Stay at 100
        { duration: '2m', target: 200 },  // Ramp to 200
        { duration: '3m', target: 200 },  // Stay at 200
        { duration: '2m', target: 0 },    // Ramp down
      ],
      gracefulRampDown: '30s',
      startTime: '6m',  // Start after load test
    },
    
    // Spike test - sudden traffic surge
    spike: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '10s', target: 200 },  // Sudden spike to 200
        { duration: '1m', target: 200 },   // Hold
        { duration: '10s', target: 0 },    // Drop
      ],
      gracefulRampDown: '10s',
      startTime: '15m',  // Start after stress test
    },
  },
  
  // Performance thresholds
  thresholds: {
    http_req_duration: ['p(50)<500', 'p(90)<1000', 'p(95)<2000'],  // Response times
    http_req_failed: ['rate<0.05'],  // Error rate < 5%
    errors: ['rate<0.1'],  // Custom error rate < 10%
    auth_success: ['rate>0.95'],  // Auth success rate > 95%
    http_reqs: ['rate>100'],  // At least 100 requests per second
  },
  
  // Summary output
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
};

// ============================================================================
// Test Data
// ============================================================================

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

// Generate unique test users
function generateUser() {
  const id = Math.random().toString(36).substring(7);
  return {
    email: `loadtest_${id}@example.com`,
    password: 'LoadTestPassword123!',
    name: `Load Test User ${id}`,
  };
}

// ============================================================================
// Helper Functions
// ============================================================================

function registerUser() {
  const user = generateUser();
  
  const payload = JSON.stringify({
    email: user.email,
    password: user.password,
    name: user.name,
  });
  
  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };
  
  const res = http.post(`${BASE_URL}/api/v1/auth/register`, payload, params);
  
  const success = check(res, {
    'register: status is 201': (r) => r.status === 201,
    'register: has user object': (r) => JSON.parse(r.body).user !== undefined,
    'register: has access token': (r) => JSON.parse(r.body).access_token !== undefined,
  });
  
  errorRate.add(!success);
  authSuccessRate.add(success);
  
  if (res.status === 201) {
    return {
      user,
      token: JSON.parse(res.body).access_token,
    };
  }
  
  return null;
}

function loginUser(email, password) {
  const payload = JSON.stringify({
    email,
    password,
  });
  
  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };
  
  const res = http.post(`${BASE_URL}/api/v1/auth/login`, payload, params);
  
  const success = check(res, {
    'login: status is 200': (r) => r.status === 200,
    'login: has access token': (r) => JSON.parse(r.body).access_token !== undefined,
  });
  
  errorRate.add(!success);
  authSuccessRate.add(success);
  
  if (res.status === 200) {
    return JSON.parse(res.body).access_token;
  }
  
  return null;
}

function getMe(token) {
  const params = {
    headers: {
      'Authorization': `Bearer ${token}`,
    },
  };
  
  const res = http.get(`${BASE_URL}/api/v1/auth/me`, params);
  
  const success = check(res, {
    'getMe: status is 200': (r) => r.status === 200,
    'getMe: has email': (r) => JSON.parse(r.body).email !== undefined,
  });
  
  errorRate.add(!success);
  
  return res;
}

function healthCheck() {
  const res = http.get(`${BASE_URL}/health`);
  
  const success = check(res, {
    'health: status is 200': (r) => r.status === 200,
    'health: is healthy': (r) => JSON.parse(r.body).status === 'healthy',
  });
  
  errorRate.add(!success);
  
  return res;
}

// ============================================================================
// Main Test Function
// ============================================================================

export default function () {
  // Scenario 1: Health check (lightweight)
  healthCheck();
  sleep(1);
  
  // Scenario 2: User registration
  const registration = registerUser();
  sleep(1);
  
  if (registration) {
    // Scenario 3: Get current user
    getMe(registration.token);
    sleep(1);
    
    // Scenario 4: Multiple authenticated requests
    for (let i = 0; i < 3; i++) {
      getMe(registration.token);
      sleep(0.5);
    }
  }
  
  // Scenario 5: Login with existing user
  if (registration) {
    const loginToken = loginUser(registration.user.email, registration.user.password);
    if (loginToken) {
      getMe(loginToken);
      sleep(1);
    }
  }
  
  sleep(2);
}

// ============================================================================
// Setup and Teardown
// ============================================================================

export function setup() {
  console.log('Starting load test...');
  console.log(`Base URL: ${BASE_URL}`);
  
  // Initial health check
  const healthRes = http.get(`${BASE_URL}/health`);
  if (healthRes.status !== 200) {
    throw new Error('Server is not healthy before starting tests');
  }
  
  return { startTime: new Date() };
}

export function teardown(data) {
  console.log('Load test completed!');
  console.log(`Total duration: ${new Date() - data.startTime}ms`);
}

// ============================================================================
// Custom Checks
// ============================================================================

export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
    'load-test-results.json': JSON.stringify(data),
  };
}

function textSummary(data, options) {
  const { indent = '', enableColors = false } = options;
  
  const metrics = data.metrics;
  const httpReqs = metrics.http_reqs ? metrics.http_reqs.values.count : 0;
  const httpReqDuration = metrics.http_req_duration ? metrics.http_req_duration.values : {};
  const errorRate = metrics.errors ? metrics.errors.values.rate : 0;
  const authSuccess = metrics.auth_success ? metrics.auth_success.values.rate : 0;
  
  return `
Load Test Summary:
${indent}Total Requests: ${httpReqs}
${indent}Request Duration:
${indent}  - Average: ${httpReqDuration.avg?.toFixed(2) || 'N/A'}ms
${indent}  - Minimum: ${httpReqDuration.min?.toFixed(2) || 'N/A'}ms
${indent}  - Maximum: ${httpReqDuration.max?.toFixed(2) || 'N/A'}ms
${indent}  - 90th percentile: ${httpReqDuration['p(90)']?.toFixed(2) || 'N/A'}ms
${indent}  - 95th percentile: ${httpReqDuration['p(95)']?.toFixed(2) || 'N/A'}ms
${indent}Error Rate: ${(errorRate * 100).toFixed(2)}%
${indent}Auth Success Rate: ${(authSuccess * 100).toFixed(2)}%
`;
}