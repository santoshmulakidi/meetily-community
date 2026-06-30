# Meetily Community+ Server Dockerfile
# Multi-stage build for production optimization

# =============================================================================
# Stage 1: Builder - Compile the Rust application
# =============================================================================
FROM rust:1.79-bookworm AS builder

# Set working directory
WORKDIR /app

# Install dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests first (for better caching)
COPY server/Cargo.toml server/Cargo.lock* ./server/

# Create a dummy main.rs to build dependencies
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs

# Build dependencies (this layer caches until Cargo.toml changes)
WORKDIR /app/server
RUN cargo fetch

# Copy source code
COPY server/ ./

# Build the application in release mode
RUN cargo build --release

# Strip binary to reduce size
RUN strip target/release/meetily-server

# =============================================================================
# Stage 2: Runtime - Minimal production image
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/* \
    && adduser --disabled-password --gecos '' meetily

# Create app directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder --chown=meetily:meetily /app/server/target/release/meetily-server /usr/local/bin/meetily-server

# Copy migrations
COPY --from=builder --chown=meetily:meetily /app/server/migrations /app/migrations

# Copy example env file
COPY --from=builder --chown=meetily:meetily /app/server/.env.example /app/.env.example

# Set environment variables
ENV RUST_LOG=info
ENV DATABASE_URL=postgresql://meetily:meetily@postgres:5432/meetily
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8080

# Create data directory for recordings
RUN mkdir -p /var/meetily/recordings && chown meetily:meetily /var/meetily/recordings
VOLUME /var/meetily/recordings

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Switch to non-root user
USER meetily

# Run the server
CMD ["meetily-server"]

# =============================================================================
# Stage 3: Development - Full development image with tools
# =============================================================================
FROM rust:1.79-bookworm AS development

WORKDIR /app

# Install development dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    curl \
    vim \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY server/ ./

# Install cargo-watch for hot reloading
RUN cargo install cargo-watch

# Expose port and debugger
EXPOSE 8080

# Default command: run with cargo-watch for hot reloading
CMD ["cargo", "watch", "-x", "run"]