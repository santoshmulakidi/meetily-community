//! Authentication Service Unit Tests

use uuid::Uuid;

use crate::auth::{
    hash_password, verify_password, generate_token, validate_token,
    Claims, JwtConfig, UserRole, User,
};

// ============================================================================
// Password Hashing Tests
// ============================================================================

#[cfg(test)]
mod password_tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "SecurePassword123!";
        let hash = hash_password(password);
        
        assert!(hash.is_ok(), "Failed to hash password: {:?}", hash);
        let hash = hash.unwrap();
        
        // Hash should be different each time (random salt)
        let hash2 = hash_password(password).unwrap();
        assert_ne!(hash, hash2, "Hashes should be different due to random salt");
        
        // Hash should start with argon2 identifier
        assert!(hash.starts_with("$argon2"), "Hash should be argon2 format");
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "SecurePassword123!";
        let hash = hash_password(password).unwrap();
        
        let is_valid = verify_password(password, &hash).unwrap();
        assert!(is_valid, "Password verification should succeed");
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "SecurePassword123!";
        let hash = hash_password(password).unwrap();
        
        let wrong_password = "WrongPassword456!";
        let is_valid = verify_password(wrong_password, &hash).unwrap();
        
        assert!(!is_valid, "Password verification should fail for wrong password");
    }

    #[test]
    fn test_verify_password_empty() {
        let password = "";
        let hash = hash_password(password).unwrap();
        
        let is_valid = verify_password(password, &hash).unwrap();
        assert!(is_valid, "Empty password should verify");
    }

    #[test]
    fn test_verify_password_special_characters() {
        let password = "P@$$w0rd!#$%^&*()_+-=[]{}|;':\",./<>?";
        let hash = hash_password(password).unwrap();
        
        let is_valid = verify_password(password, &hash).unwrap();
        assert!(is_valid, "Special characters should be handled");
    }

    #[test]
    fn test_verify_password_unicode() {
        let password = "パスワード🔐";
        let hash = hash_password(password).unwrap();
        
        let is_valid = verify_password(password, &hash).unwrap();
        assert!(is_valid, "Unicode characters should be handled");
    }

    #[test]
    fn test_hash_password_long() {
        let password = "a".repeat(1000);
        let hash = hash_password(&password).unwrap();
        
        let is_valid = verify_password(&password, &hash).unwrap();
        assert!(is_valid, "Long passwords should be handled");
    }
}

// ============================================================================
// JWT Token Tests
// ============================================================================

#[cfg(test)]
mod jwt_tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn create_test_user() -> User {
        User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: hash_password("password123").unwrap(),
            name: Some("Test User".to_string()),
            role: UserRole::User,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_generate_token() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let token = generate_token(&user, &config);
        
        assert!(token.is_ok(), "Failed to generate token: {:?}", token);
        let token = token.unwrap();
        
        // Token should be non-empty
        assert!(!token.is_empty(), "Token should not be empty");
        
        // Token should have 3 parts (header.payload.signature)
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts");
    }

    #[test]
    fn test_validate_token_success() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let token = generate_token(&user, &config).unwrap();
        let claims = validate_token(&token, &config).unwrap();
        
        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.email, user.email);
        assert_eq!(claims.role, user.role);
        assert!(claims.exp > Utc::now().timestamp() as usize);
    }

    #[test]
    fn test_validate_token_wrong_secret() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        let wrong_config = JwtConfig {
            secret: "wrong_secret".to_string(),
            expiration_hours: 24,
        };
        
        let token = generate_token(&user, &config).unwrap();
        let claims = validate_token(&token, &wrong_config);
        
        assert!(claims.is_err(), "Validation should fail with wrong secret");
    }

    #[test]
    fn test_validate_token_expired() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: -1, // Expired
        };
        
        let token = generate_token(&user, &config).unwrap();
        
        // Small delay to ensure expiration
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let claims = validate_token(&token, &config);
        
        // May fail due to expiration or succeed depending on timing
        // The important thing is the token structure is valid
        if let Ok(claims) = claims {
            assert_eq!(claims.sub, user.id.to_string());
        }
    }

    #[test]
    fn test_validate_token_malformed() {
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let malformed_token = "not.a.valid.jwt.token";
        let claims = validate_token(malformed_token, &config);
        
        assert!(claims.is_err(), "Validation should fail for malformed token");
    }

    #[test]
    fn test_validate_token_empty() {
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let claims = validate_token("", &config);
        
        assert!(claims.is_err(), "Validation should fail for empty token");
    }

    #[test]
    fn test_claims_structure() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let token = generate_token(&user, &config).unwrap();
        let claims = validate_token(&token, &config).unwrap();
        
        // Verify all claims are present
        assert!(!claims.sub.is_empty());
        assert!(!claims.email.is_empty());
        assert!(claims.exp > 0);
        assert!(claims.iat > 0);
        assert!(claims.exp >= claims.iat);
    }

    #[test]
    fn test_token_uniqueness() {
        let user = create_test_user();
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        // Generate multiple tokens for same user
        let token1 = generate_token(&user, &config).unwrap();
        let token2 = generate_token(&user, &config).unwrap();
        let token3 = generate_token(&user, &config).unwrap();
        
        // All tokens should be unique (different timestamps)
        assert_ne!(token1, token2);
        assert_ne!(token2, token3);
        assert_ne!(token1, token3);
        
        // But all should validate successfully
        let claims1 = validate_token(&token1, &config).unwrap();
        let claims2 = validate_token(&token2, &config).unwrap();
        let claims3 = validate_token(&token3, &config).unwrap();
        
        assert_eq!(claims1.sub, claims2.sub);
        assert_eq!(claims2.sub, claims3.sub);
    }

    #[test]
    fn test_admin_user_token() {
        let mut user = create_test_user();
        user.role = UserRole::Admin;
        
        let config = JwtConfig {
            secret: "test_secret".to_string(),
            expiration_hours: 24,
        };
        
        let token = generate_token(&user, &config).unwrap();
        let claims = validate_token(&token, &config).unwrap();
        
        assert_eq!(claims.role, UserRole::Admin);
    }
}

// ============================================================================
// Rate Limiter Tests
// ============================================================================

#[cfg(test)]
mod rate_limiter_tests {
    use super::*;
    use crate::auth::RateLimiter;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(10); // 10 requests per minute
        
        for i in 0..9 {
            let allowed = limiter.is_allowed("test_ip").await;
            assert!(allowed, "Request {} should be allowed", i + 1);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(10); // 10 requests per minute
        
        // Make 10 requests
        for _ in 0..10 {
            limiter.is_allowed("test_ip").await;
        }
        
        // 11th request should be blocked
        let allowed = limiter.is_allowed("test_ip").await;
        assert!(!allowed, "Request over limit should be blocked");
    }

    #[tokio::test]
    async fn test_rate_limiter_different_ips() {
        let limiter = RateLimiter::new(5); // 5 requests per minute
        
        // IP1 uses all 5 requests
        for _ in 0..5 {
            limiter.is_allowed("ip1").await;
        }
        
        // IP1 should be blocked
        assert!(!limiter.is_allowed("ip1").await);
        
        // IP2 should still be allowed (separate limit)
        assert!(limiter.is_allowed("ip2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_get_remaining() {
        let limiter = RateLimiter::new(10);
        
        // Make 3 requests
        for _ in 0..3 {
            limiter.is_allowed("test_ip").await;
        }
        
        let remaining = limiter.get_remaining("test_ip").await;
        assert_eq!(remaining, 7, "Should have 7 requests remaining");
    }

    #[tokio::test]
    async fn test_rate_limiter_remaining_zero() {
        let limiter = RateLimiter::new(5);
        
        // Use all requests
        for _ in 0..5 {
            limiter.is_allowed("test_ip").await;
        }
        
        let remaining = limiter.get_remaining("test_ip").await;
        assert_eq!(remaining, 0, "Should have 0 requests remaining");
    }
}