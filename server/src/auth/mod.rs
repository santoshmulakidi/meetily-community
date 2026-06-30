//! JWT Authentication middleware and utilities
//!
//! Features:
//! - JWT token generation and validation
//! - Password hashing with Argon2
//! - User management (registration, login)
//! - API key authentication
//! - Rate limiting
//! - Role-based access control (RBAC)

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{AppError, ServiceResult};

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Email
    pub email: String,
    /// Role (admin, user)
    pub role: UserRole,
    /// Expiration time
    pub exp: usize,
    /// Issued at
    pub iat: usize,
}

/// User roles for RBAC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    User,
}

/// User registration request
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

/// User login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Authentication response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserResponse,
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// User response (without sensitive data)
#[derive(Debug, Serialize, Clone)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database user model
#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub name: Option<String>,
    pub role: UserRole,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// JWT Configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

impl JwtConfig {
    pub fn from_env() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "default_secret_change_in_production".to_string()),
            expiration_hours: 24,
        }
    }
}

/// Rate limiter state
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Requests per minute limit
    pub requests_per_minute: u32,
    /// In-memory store: IP -> (count, reset_time)
    store: Arc<RwLock<std::collections::HashMap<String, (u32, i64)>>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            store: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Check if request is allowed
    pub async fn is_allowed(&self, key: &str) -> bool {
        let now = Utc::now().timestamp();
        let mut store = self.store.write().await;
        
        let (count, reset_time) = store.entry(key.to_string()).or_insert((0, now + 60));
        
        // Reset if minute has passed
        if now >= *reset_time {
            *count = 0;
            *reset_time = now + 60;
        }
        
        if *count >= self.requests_per_minute {
            return false;
        }
        
        *count += 1;
        true
    }
    
    /// Get remaining requests
    pub async fn get_remaining(&self, key: &str) -> u32 {
        let now = Utc::now().timestamp();
        let store = self.store.read().await;
        
        if let Some((count, reset_time)) = store.get(key) {
            if now >= *reset_time {
                self.requests_per_minute
            } else {
                self.requests_per_minute.saturating_sub(*count)
            }
        } else {
            self.requests_per_minute
        }
    }
}

/// Hash password with Argon2
pub fn hash_password(password: &str) -> ServiceResult<String> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
        Argon2,
    };
    
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::AuthenticationError(format!("Failed to hash password: {}", e)))?
        .to_string();
    
    Ok(password_hash)
}

/// Verify password against hash
pub fn verify_password(password: &str, hash: &str) -> ServiceResult<bool> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };
    
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::AuthenticationError(format!("Invalid password hash: {}", e)))?;
    
    let is_valid = Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();
    
    Ok(is_valid)
}

/// Generate JWT token
pub fn generate_token(user: &User, config: &JwtConfig) -> ServiceResult<String> {
    let now = Utc::now();
    let exp = now + Duration::hours(config.expiration_hours);
    
    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| AppError::AuthenticationError(format!("Failed to generate token: {}", e)))?;
    
    Ok(token)
}

/// Validate JWT token
pub fn validate_token(token: &str, config: &JwtConfig) -> ServiceResult<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::AuthenticationError(format!("Invalid token: {}", e)))?;
    
    Ok(token_data.claims)
}

/// Extract user from JWT token in request
pub async fn extract_user_from_token(
    token: &str,
    config: &JwtConfig,
    db_pool: &Pool<Postgres>,
) -> ServiceResult<User> {
    // Validate token
    let claims = validate_token(token, config)?;
    
    // Check expiration
    if claims.exp < Utc::now().timestamp() as usize {
        return Err(AppError::AuthenticationError("Token expired".to_string()));
    }
    
    // Get user from database
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthenticationError("Invalid user ID in token".to_string()))?;
    
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_optional(db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?
    .ok_or_else(|| AppError::AuthenticationError("User not found".to_string()))?;
    
    Ok(user)
}

/// JWT Authentication middleware
pub async fn auth_middleware<B>(
    State(db_pool): State<Arc<Pool<Postgres>>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let config = JwtConfig::from_env();
    
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing Authorization header");
            StatusCode::UNAUTHORIZED
        })?;
    
    // Parse Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            tracing::warn!("Invalid Authorization header format");
            StatusCode::UNAUTHORIZED
        })?;
    
    // Validate token and get user
    let user = extract_user_from_token(token, &config, &db_pool)
        .await
        .map_err(|e| {
            tracing::warn!("Authentication failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    // Insert user into request extensions
    request.extensions_mut().insert(user);
    
    Ok(next.run(request).await)
}

/// Admin-only middleware
pub async fn admin_middleware<B>(
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Get user from request extensions (set by auth_middleware)
    let user = request
        .extensions()
        .get::<User>()
        .ok_or_else(|| {
            tracing::warn!("User not found in request extensions");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Check role
    if user.role != UserRole::Admin {
        tracing::warn!("User {} attempted admin action with role {:?}", user.email, user.role);
        return Err(StatusCode::FORBIDDEN);
    }
    
    Ok(next.run(request).await)
}

/// Rate limiting middleware
pub async fn rate_limit_middleware<B>(
    State(limiter): State<Arc<RateLimiter>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Get client IP
    let client_ip = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    // Check rate limit
    if !limiter.is_allowed(&client_ip).await {
        tracing::warn!("Rate limit exceeded for IP: {}", client_ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    // Add rate limit headers to response
    let remaining = limiter.get_remaining(&client_ip).await;
    let mut response = next.run(request).await;
    
    response.headers_mut().insert(
        "X-RateLimit-Remaining",
        axum::http::HeaderValue::from_str(&remaining.to_string()).unwrap(),
    );
    
    Ok(response)
}

/// Register a new user
pub async fn register_user(
    db_pool: &Pool<Postgres>,
    request: RegisterRequest,
) -> ServiceResult<AuthResponse> {
    // Check if user already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1",
    )
    .bind(&request.email)
    .fetch_one(db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    if existing > 0 {
        return Err(AppError::ConflictError("User with this email already exists".to_string()));
    }
    
    // Hash password
    let password_hash = hash_password(&request.password)?;
    
    // Create user
    let user_id = Uuid::new_v4();
    let now = Utc::now();
    
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, email, password_hash, name, role, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&request.email)
    .bind(&password_hash)
    .bind(&request.name)
    .bind(UserRole::User)  // Default role
    .bind(true)
    .bind(now)
    .bind(now)
    .fetch_one(db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    // Generate token
    let config = JwtConfig::from_env();
    let token = generate_token(&user, &config)?;
    
    Ok(AuthResponse {
        user: UserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role,
            created_at: user.created_at,
        },
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: config.expiration_hours * 3600,
    })
}

/// Login user
pub async fn login_user(
    db_pool: &Pool<Postgres>,
    request: LoginRequest,
) -> ServiceResult<AuthResponse> {
    // Get user by email
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = true",
    )
    .bind(&request.email)
    .fetch_optional(db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?
    .ok_or_else(|| AppError::AuthenticationError("Invalid email or password".to_string()))?;
    
    // Verify password
    if !verify_password(&request.password, &user.password_hash)? {
        return Err(AppError::AuthenticationError("Invalid email or password".to_string()));
    }
    
    // Generate token
    let config = JwtConfig::from_env();
    let token = generate_token(&user, &config)?;
    
    Ok(AuthResponse {
        user: UserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role,
            created_at: user.created_at,
        },
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: config.expiration_hours * 3600,
    })
}