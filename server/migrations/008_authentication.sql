-- Migration: Create users and API keys tables for authentication
-- Version: 008

-- User roles enum
CREATE TYPE user_role AS ENUM ('admin', 'user');

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    name TEXT,
    role user_role NOT NULL DEFAULT 'user',
    is_active BOOLEAN NOT NULL DEFAULT true,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- API keys table (for programmatic access)
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,
    name TEXT,
    description TEXT,
    scopes JSONB NOT NULL DEFAULT '["read", "write"]',  -- Permissions
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rate limit tracking (optional - can also use in-memory)
CREATE TABLE IF NOT EXISTS rate_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identifier TEXT NOT NULL,  -- IP address or user ID
    endpoint TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    window_start TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(identifier, endpoint, window_start)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);
CREATE INDEX IF NOT EXISTS idx_users_is_active ON users(is_active);
CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_is_active ON api_keys(is_active);
CREATE INDEX IF NOT EXISTS idx_rate_limits_identifier ON rate_limits(identifier);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window_start ON rate_limits(window_start);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_api_keys_updated_at
    BEFORE UPDATE ON api_keys
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Comments
COMMENT ON TABLE users IS 'User accounts for authentication';
COMMENT ON TABLE api_keys IS 'API keys for programmatic access';
COMMENT ON COLUMN users.role IS 'User role: admin or user (RBAC)';
COMMENT ON COLUMN users.email_verified IS 'Whether email has been verified';
COMMENT ON COLUMN api_keys.scopes IS 'JSON array of permissions: ["read", "write", "admin"]';
COMMENT ON COLUMN api_keys.expires_at IS 'Key expiration (NULL = never expires)';
COMMENT ON COLUMN api_keys.last_used_at IS 'Last time this key was used';

-- View: Active users with stats
CREATE OR REPLACE VIEW v_active_users AS
SELECT 
    u.id,
    u.email,
    u.name,
    u.role,
    u.created_at,
    u.last_login_at,
    COUNT(DISTINCT m.id) as meetings_count,
    COUNT(DISTINCT ak.id) as api_keys_count
FROM users u
LEFT JOIN meetings m ON u.id = m.user_id  -- Assuming meetings will have user_id in future
LEFT JOIN api_keys ak ON u.id = ak.user_id AND ak.is_active = true
WHERE u.is_active = true
GROUP BY u.id, u.email, u.name, u.role, u.created_at, u.last_login_at
ORDER BY u.created_at DESC;

-- Function: Create admin user (for initial setup)
CREATE OR REPLACE FUNCTION create_admin_user(
    p_email TEXT,
    p_password_hash TEXT,
    p_name TEXT DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_user_id UUID;
BEGIN
    INSERT INTO users (id, email, password_hash, name, role, is_active, email_verified, created_at, updated_at)
    VALUES (
        gen_random_uuid(),
        p_email,
        p_password_hash,
        p_name,
        'admin'::user_role,
        true,
        true,
        NOW(),
        NOW()
    )
    RETURNING id INTO v_user_id;
    
    RETURN v_user_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Grant permissions
-- GRANT SELECT, INSERT, UPDATE ON users TO meetily_app;
-- GRANT SELECT, INSERT, UPDATE ON api_keys TO meetily_app;
-- GRANT EXECUTE ON FUNCTION create_admin_user TO meetily_app;