//! Authentication API handlers
//!
//! Endpoints:
//! - POST /api/v1/auth/register - Register new user
//! - POST /api/v1/auth/login - Login user
//! - POST /api/v1/auth/logout - Logout (invalidate token)
//! - GET /api/v1/auth/me - Get current user
//! - PUT /api/v1/auth/me - Update current user
//! - POST /api/v1/auth/refresh - Refresh JWT token
//! - POST /api/v1/auth/password/reset - Request password reset
//! - POST /api/v1/auth/password/change - Change password

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{
    self,
    Claims, JwtConfig, LoginRequest, RegisterRequest, AuthResponse, 
    User, UserResponse, UserRole,
};
use crate::error::{AppError, ServiceResult};

/// Auth state (shared state for auth handlers)
#[derive(Clone)]
pub struct AuthState {
    pub db_pool: Arc<Pool<Postgres>>,
    pub jwt_config: JwtConfig,
}

/// Register new user
///
/// POST /api/v1/auth/register
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "User already exists"),
    ),
)]
pub async fn register(
    State(db_pool): State<Arc<Pool<Postgres>>>,
    Json(request): Json<RegisterRequest>,
) -> ServiceResult<Json<AuthResponse>> {
    // Validate email format
    if !request.email.contains('@') {
        return Err(AppError::ValidationError("Invalid email format".to_string()));
    }
    
    // Validate password strength
    if request.password.len() < 8 {
        return Err(AppError::ValidationError(
            "Password must be at least 8 characters".to_string()
        ));
    }
    
    // Register user
    let response = auth::register_user(&db_pool, request).await?;
    
    Ok(Json(response))
}

/// Login user
///
/// POST /api/v1/auth/login
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(
    State(db_pool): State<Arc<Pool<Postgres>>>,
    Json(request): Json<LoginRequest>,
) -> ServiceResult<Json<AuthResponse>> {
    let response = auth::login_user(&db_pool, request).await?;
    
    // Update last_login_at
    let _ = sqlx::query!(
        "UPDATE users SET last_login_at = NOW() WHERE id = $1",
        response.user.id
    )
    .execute(&*db_pool)
    .await;
    
    Ok(Json(response))
}

/// Get current user
///
/// GET /api/v1/auth/me
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    security(("ApiKeyAuth" = [])),
    responses(
        (status = 200, description = "Current user info", body = UserResponse),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub async fn get_current_user(
    Extension(user): Extension<User>,
) -> ServiceResult<Json<UserResponse>> {
    Ok(Json(UserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
        created_at: user.created_at,
    }))
}

/// Update current user
///
/// PUT /api/v1/auth/me
#[utoipa::path(
    put,
    path = "/api/v1/auth/me",
    tag = "auth",
    security(("ApiKeyAuth" = [])),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserResponse),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub async fn update_current_user(
    Extension(user): Extension<User>,
    State(db_pool): State<Arc<Pool<Postgres>>>,
    Json(request): Json<UpdateUserRequest>,
) -> ServiceResult<Json<UserResponse>> {
    let updated_user = sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET name = COALESCE($1, name),
            updated_at = NOW()
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(&request.name)
    .bind(user.id)
    .fetch_one(&*db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    Ok(Json(UserResponse {
        id: updated_user.id,
        email: updated_user.email,
        name: updated_user.name,
        role: updated_user.role,
        created_at: updated_user.created_at,
    }))
}

/// Change password
///
/// PUT /api/v1/auth/password/change
#[utoipa::path(
    put,
    path = "/api/v1/auth/password/change",
    tag = "auth",
    security(("ApiKeyAuth" = [])),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed"),
        (status = 401, description = "Invalid current password"),
    ),
)]
pub async fn change_password(
    Extension(user): Extension<User>,
    State(db_pool): State<Arc<Pool<Postgres>>>,
    Json(request): Json<ChangePasswordRequest>,
) -> ServiceResult<StatusCode> {
    // Verify current password
    if !auth::verify_password(&request.current_password, &user.password_hash)? {
        return Err(AppError::AuthenticationError("Current password is incorrect".to_string()));
    }
    
    // Validate new password
    if request.new_password.len() < 8 {
        return Err(AppError::ValidationError(
            "Password must be at least 8 characters".to_string()
        ));
    }
    
    // Hash new password
    let new_hash = auth::hash_password(&request.new_password)?;
    
    // Update password
    sqlx::query!(
        "UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2",
        new_hash,
        user.id
    )
    .execute(&*db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    Ok(StatusCode::OK)
}

/// Refresh JWT token
///
/// POST /api/v1/auth/refresh
#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    security(("ApiKeyAuth" = [])),
    responses(
        (status = 200, description = "Token refreshed", body = TokenRefreshResponse),
        (status = 401, description = "Invalid token"),
    ),
)]
pub async fn refresh_token(
    Extension(user): Extension<User>,
) -> ServiceResult<Json<TokenRefreshResponse>> {
    let config = JwtConfig::from_env();
    let token = auth::generate_token(&user, &config)?;
    
    Ok(Json(TokenRefreshResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: config.expiration_hours * 3600,
    }))
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Update user request
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
}

/// Change password request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Token refresh response
#[derive(Debug, Serialize)]
pub struct TokenRefreshResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}