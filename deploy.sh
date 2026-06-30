#!/bin/bash
# Meetily Community+ Deployment Script for Oracle VM
# This script automates the deployment process on Ubuntu/Debian systems

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="meetily"
COMPOSE_FILE="docker-compose.yml"
ENV_FILE=".env"

# =============================================================================
# Helper Functions
# =============================================================================

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_command() {
    if ! command -v $1 &> /dev/null; then
        log_error "$1 is not installed. Please install it first."
        exit 1
    fi
}

# =============================================================================
# Pre-flight Checks
# =============================================================================

preflight_checks() {
    log_info "Running pre-flight checks..."
    
    # Check for required commands
    check_command docker
    check_command docker-compose
    
    # Check if running as root or with sudo
    if [ "$EUID" -ne 0 ]; then
        log_warn "Not running as root. Some commands may require sudo."
    fi
    
    # Check if .env file exists
    if [ ! -f "$ENV_FILE" ]; then
        log_warn "$ENV_FILE not found. Copying from .env.example..."
        cp .env.example $ENV_FILE
        log_warn "Please edit $ENV_FILE and set your API keys before starting."
        exit 1
    fi
    
    log_info "Pre-flight checks passed!"
}

# =============================================================================
# Build the Application
# =============================================================================

build() {
    log_info "Building Meetily server Docker image..."
    docker-compose build --no-cache
    log_info "Build complete!"
}

# =============================================================================
# Start the Application
# =============================================================================

start() {
    log_info "Starting Meetily services..."
    docker-compose up -d
    log_info "Services started!"
    
    # Wait for services to be healthy
    log_info "Waiting for services to become healthy..."
    sleep 10
    
    # Check health
    if curl -f http://localhost:8080/health &> /dev/null; then
        log_info "✅ Meetily server is running and healthy!"
        log_info "Access the dashboard at: http://$(hostname -I | awk '{print $1}'):8080"
        log_info "Swagger UI: http://$(hostname -I | awk '{print $1}'):8080/swagger-ui"
    else
        log_warn "Server may still be starting. Check logs with: docker-compose logs -f"
    fi
}

# =============================================================================
# Stop the Application
# =============================================================================

stop() {
    log_info "Stopping Meetily services..."
    docker-compose down
    log_info "Services stopped!"
}

# =============================================================================
# Restart the Application
# =============================================================================

restart() {
    stop
    sleep 2
    start
}

# =============================================================================
# View Logs
# =============================================================================

logs() {
    log_info "Showing logs (press Ctrl+C to exit)..."
    docker-compose logs -f "$@"
}

# =============================================================================
# Run Database Migrations
# =============================================================================

migrate() {
    log_info "Running database migrations..."
    docker-compose exec meetily-server meetily-server migrate
    log_info "Migrations complete!"
}

# =============================================================================
# Backup Database
# =============================================================================

backup() {
    log_info "Creating database backup..."
    BACKUP_FILE="backups/meetily_backup_$(date +%Y%m%d_%H%M%S).sql.gz"
    mkdir -p backups
    
    docker-compose exec -T postgres pg_dump -U meetily -d meetily | gzip > $BACKUP_FILE
    
    log_info "Backup created: $BACKUP_FILE"
}

# =============================================================================
# Restore Database from Backup
# =============================================================================

restore() {
    if [ -z "$1" ]; then
        log_error "Please specify a backup file to restore."
        echo "Usage: $0 restore <backup_file.sql.gz>"
        exit 1
    fi
    
    BACKUP_FILE=$1
    
    if [ ! -f "$BACKUP_FILE" ]; then
        log_error "Backup file not found: $BACKUP_FILE"
        exit 1
    fi
    
    log_warn "This will overwrite the current database. Continue? (y/n)"
    read -r response
    if [[ "$response" != "y" ]]; then
        log_info "Restore cancelled."
        exit 0
    fi
    
    log_info "Restoring database from $BACKUP_FILE..."
    gunzip -c $BACKUP_FILE | docker-compose exec -T postgres psql -U meetily -d meetily
    log_info "Database restored!"
}

# =============================================================================
# Update Application
# =============================================================================

update() {
    log_info "Updating Meetily..."
    
    # Pull latest changes (if using git)
    if [ -d ".git" ]; then
        git pull origin main
    fi
    
    # Rebuild
    build
    
    # Restart with migrations
    docker-compose down
    docker-compose up -d
    
    log_info "Update complete!"
}

# =============================================================================
# Cleanup (Remove all containers, volumes, and images)
# =============================================================================

cleanup() {
    log_warn "This will remove ALL Meetily data including database and recordings. Continue? (y/n)"
    read -r response
    if [[ "$response" != "y" ]]; then
        log_info "Cleanup cancelled."
        exit 0
    fi
    
    log_info "Stopping services..."
    docker-compose down -v --remove-orphans
    
    log_info "Removing images..."
    docker rmi meetily-server 2>/dev/null || true
    
    log_info "Cleanup complete!"
    log_warn "All Meetily data has been permanently deleted."
}

# =============================================================================
# Health Check
# =============================================================================

health() {
    log_info "Checking service health..."
    
    echo ""
    echo "=== PostgreSQL ==="
    docker-compose exec -T postgres pg_isready -U meetily -d meetily
    
    echo ""
    echo "=== Meetily Server ==="
    curl -s http://localhost:8080/health | jq .
    
    echo ""
    log_info "Health check complete!"
}

# =============================================================================
# Usage Information
# =============================================================================

usage() {
    echo "Meetily Community+ Deployment Script"
    echo ""
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  build       Build Docker images"
    echo "  start       Start all services"
    echo "  stop        Stop all services"
    echo "  restart     Restart all services"
    echo "  logs        View service logs"
    echo "  migrate     Run database migrations"
    echo "  backup      Create database backup"
    echo "  restore     Restore database from backup"
    echo "  update      Update to latest version"
    echo "  cleanup     Remove all containers and data"
    echo "  health      Check service health"
    echo "  help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 build"
    echo "  $0 start"
    echo "  $0 logs meetily-server"
    echo "  $0 backup"
    echo "  $0 restore backups/meetily_backup_20240629.sql.gz"
}

# =============================================================================
# Main Script
# =============================================================================

case "$1" in
    build)
        preflight_checks
        build
        ;;
    start)
        preflight_checks
        start
        ;;
    stop)
        stop
        ;;
    restart)
        restart
        ;;
    logs)
        logs "$2"
        ;;
    migrate)
        migrate
        ;;
    backup)
        backup
        ;;
    restore)
        restore "$2"
        ;;
    update)
        update
        ;;
    cleanup)
        cleanup
        ;;
    health)
        health
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        usage
        exit 1
        ;;
esac

exit 0