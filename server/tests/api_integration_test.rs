//! API Integration Tests
//!
//! Full integration tests for REST API endpoints.
//! Requires a running test database.
//!
//! Run with: cargo test --test api_integration_test -- --test-threads=1

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use sqlx::{PgPool, Postgres};
use std::net::SocketAddr;
use tower::ServiceExt;
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

struct TestApp {
    app: Router,
    db_pool: PgPool,
    base_url: String,
}

impl TestApp {
    async fn new() -> Self {
        // Use test database
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/meetily_test".to_string());
        
        let db_pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!()
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        
        // Create app
        let app = crate::api::create_router(db_pool.clone());
        
        Self {
            app,
            db_pool,
            base_url: "http://test.local".to_string(),
        }
    }

    async fn post_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> (StatusCode, serde_json::Value) {
        let request = Request::builder()
            .method(axum::http::Method::POST)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(body).unwrap()))
            .unwrap();
        
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        
        let body = axum::body::to_bytes(response.into_body(), 10_000_000)
            .await
            .unwrap();
        
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        
        (status, json)
    }

    async fn get_json(&self, path: &str, token: Option<&str>) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder()
            .method(axum::http::Method::GET)
            .uri(path);
        
        if let Some(token) = token {
            request = request.header(
                axum::http::header::AUTHORIZATION,
                format!("Bearer {}", token),
            );
        }
        
        let request = request.body(Body::empty()).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        
        let body = axum::body::to_bytes(response.into_body(), 10_000_000)
            .await
            .unwrap();
        
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        
        (status, json)
    }
}

// ============================================================================
// Authentication Integration Tests
// ============================================================================

#[cfg(test)]
mod auth_integration {
    use super::*;

    #[tokio::test]
    async fn test_register_user_success() {
        let app = TestApp::new().await;
        
        let email = format!("test_{}@example.com", Uuid::new_v4());
        let body = json!({
            "email": email,
            "password": "SecurePassword123!",
            "name": "Test User"
        });
        
        let (status, response) = app.post_json("/api/v1/auth/register", &body).await;
        
        assert_eq!(status, StatusCode::CREATED, "Registration should succeed");
        assert!(response["user"]["id"].is_string());
        assert_eq!(response["user"]["email"], email);
        assert_eq!(response["user"]["role"], "user");
        assert!(response["access_token"].is_string());
        assert_eq!(response["token_type"], "Bearer");
        assert!(response["expires_in"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_register_user_duplicate_email() {
        let app = TestApp::new().await;
        
        let email = format!("dup_{}@example.com", Uuid::new_v4());
        let body = json!({
            "email": email,
            "password": "SecurePassword123!",
            "name": "Test User"
        });
        
        // First registration
        let (status, _) = app.post_json("/api/v1/auth/register", &body).await;
        assert_eq!(status, StatusCode::CREATED);
        
        // Duplicate registration
        let (status, response) = app.post_json("/api/v1/auth/register", &body).await;
        
        assert_eq!(status, StatusCode::CONFLICT, "Duplicate email should fail");
        assert!(response["error"].is_string());
    }

    #[tokio::test]
    async fn test_register_user_weak_password() {
        let app = TestApp::new().await;
        
        let email = format!("weak_{}@example.com", Uuid::new_v4());
        let body = json!({
            "email": email,
            "password": "weak", // Too short
            "name": "Test User"
        });
        
        let (status, response) = app.post_json("/api/v1/auth/register", &body).await;
        
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response["error"].is_string());
    }

    #[tokio::test]
    async fn test_login_success() {
        let app = TestApp::new().await;
        
        let email = format!("login_{}@example.com", Uuid::new_v4());
        let password = "SecurePassword123!";
        
        // Register first
        let register_body = json!({
            "email": email,
            "password": password,
            "name": "Test User"
        });
        app.post_json("/api/v1/auth/register", &register_body).await;
        
        // Login
        let login_body = json!({
            "email": email,
            "password": password
        });
        let (status, response) = app.post_json("/api/v1/auth/login", &login_body).await;
        
        assert_eq!(status, StatusCode::OK, "Login should succeed");
        assert!(response["access_token"].is_string());
        assert_eq!(response["user"]["email"], email);
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let app = TestApp::new().await;
        
        let email = format!("wrong_{}@example.com", Uuid::new_v4());
        let password = "SecurePassword123!";
        
        // Register first
        let register_body = json!({
            "email": email,
            "password": password,
            "name": "Test User"
        });
        app.post_json("/api/v1/auth/register", &register_body).await;
        
        // Login with wrong password
        let login_body = json!({
            "email": email,
            "password": "WrongPassword456!"
        });
        let (status, _) = app.post_json("/api/v1/auth/login", &login_body).await;
        
        assert_eq!(status, StatusCode::UNAUTHORIZED, "Wrong password should fail");
    }

    #[tokio::test]
    async fn test_get_current_user() {
        let app = TestApp::new().await;
        
        // Register and get token
        let email = format!("me_{}@example.com", Uuid::new_v4());
        let password = "SecurePassword123!";
        
        let register_body = json!({
            "email": email,
            "password": password,
            "name": "Test User"
        });
        let (_, response) = app.post_json("/api/v1/auth/register", &register_body).await;
        let token = response["access_token"].as_str().unwrap();
        
        // Get current user
        let (status, response) = app.get_json("/api/v1/auth/me", Some(token)).await;
        
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["email"], email);
        assert_eq!(response["name"], "Test User");
    }

    #[tokio::test]
    async fn test_get_current_user_unauthorized() {
        let app = TestApp::new().await;
        
        // Try without token
        let (status, _) = app.get_json("/api/v1/auth/me", None).await;
        
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_refresh_token() {
        let app = TestApp::new().await;
        
        // Register and get token
        let email = format!("refresh_{}@example.com", Uuid::new_v4());
        let password = "SecurePassword123!";
        
        let register_body = json!({
            "email": email,
            "password": password,
            "name": "Test User"
        });
        let (_, response) = app.post_json("/api/v1/auth/register", &register_body).await;
        let old_token = response["access_token"].as_str().unwrap();
        
        // Refresh token
        let (status, response) = app.get_json("/api/v1/auth/refresh", Some(old_token)).await;
        
        assert_eq!(status, StatusCode::OK);
        assert!(response["access_token"].is_string());
        assert_ne!(response["access_token"].as_str().unwrap(), old_token);
    }
}

// ============================================================================
// Rate Limiting Integration Tests
// ============================================================================

#[cfg(test)]
mod rate_limit_integration {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting() {
        let app = TestApp::new().await;
        
        let email = format!("ratelimit_{}@example.com", Uuid::new_v4());
        let body = json!({
            "email": email,
            "password": "SecurePassword123!",
            "name": "Test User"
        });
        
        // Make many requests quickly
        let mut success_count = 0;
        let mut rate_limited_count = 0;
        
        for _ in 0..15 {
            let (status, _) = app.post_json("/api/v1/auth/register", &body).await;
            
            match status {
                StatusCode::CREATED | StatusCode::CONFLICT => success_count += 1,
                StatusCode::TOO_MANY_REQUESTS => rate_limited_count += 1,
                _ => panic!("Unexpected status: {}", status),
            }
        }
        
        // Should have some rate limited requests
        assert!(rate_limited_count > 0, "Should hit rate limit after many requests");
        println!("Success: {}, Rate Limited: {}", success_count, rate_limited_count);
    }
}

// ============================================================================
// Health Check Tests
// ============================================================================

#[cfg(test)]
mod health_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = TestApp::new().await;
        
        let (status, response) = app.get_json("/health", None).await;
        
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["status"], "healthy");
        assert!(response["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_ready_endpoint() {
        let app = TestApp::new().await;
        
        let (status, response) = app.get_json("/ready", None).await;
        
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["ready"], true);
        assert!(response["database"].is_boolean());
    }
}