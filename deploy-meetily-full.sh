#!/bin/bash
# Full Meetily Backend Deployment to Oracle VM
# This script:
#   1. Clones your Meetily repo to the VM
#   2. Configures Docker to use port 8082
#   3. Starts the backend
#   4. Verifies it's running

set -e

VM_IP="163.192.111.51"
VM_USER="ubuntu"
SSH_KEY="$HOME/.ssh/ssh-key-2026-05-28.key"
REPO_URL="https://github.com/Zackriya-Solutions/meetily.git"

echo "🚀 Meetily Full Deployment Script"
echo "==================================="
echo ""
echo "Target: $VM_USER@$VM_IP"
echo "SSH Key: $SSH_KEY"
echo "Repository: $REPO_URL"
echo "Port: 8082"
echo ""

# Test SSH
echo "📡 Testing SSH connection..."
if ! ssh -i $SSH_KEY -o ConnectTimeout=10 -o BatchMode=yes $VM_USER@$VM_IP "exit" 2>/dev/null; then
    echo "❌ SSH connection failed!"
    exit 1
fi
echo "✅ SSH connection successful!"
echo ""

# Deploy to VM
echo "🔧 Deploying Meetily to Oracle VM..."
ssh -i $SSH_KEY $VM_USER@$VM_IP << 'ENDSSH'
#!/bin/bash
set -e

echo "Step 1/6: Checking if meetily-community exists..."
if [ -d ~/meetily-community ]; then
    echo "✅ meetily-community already exists"
    cd ~/meetily-community
    
    echo "Step 2/6: Pulling latest changes..."
    git pull || echo "⚠️  Git pull failed (may not be a git repo)"
else
    echo "📥 Cloning Meetily repository..."
    git clone $REPO_URL meetily-community || {
        echo "❌ Failed to clone repository"
        echo "Trying with HTTPS..."
        git clone https://github.com/Zackriya-Solutions/meetily.git meetily-community
    }
    cd ~/meetily-community
fi

echo ""
echo "Step 3/6: Checking docker-compose.yml..."
if [ ! -f docker-compose.yml ]; then
    echo "❌ docker-compose.yml not found!"
    echo "Current directory contents:"
    ls -la
    exit 1
fi

# Backup and update port
cp docker-compose.yml docker-compose.yml.backup.$(date +%Y%m%d_%H%M%S)

echo ""
echo "Step 4/6: Configuring port 8082..."
if grep -q '"8080:8080"' docker-compose.yml; then
    sed -i 's/"8080:8080"/"8082:8080"/g' docker-compose.yml
    echo "✅ Port updated: 8080 → 8082"
else
    echo "⚠️  Port 8080:8080 not found (may already be customized)"
fi

# Check if .env file exists
echo ""
echo "Step 5/6: Setting up environment..."
if [ ! -f .env ]; then
    echo "⚠️  Creating .env file from .env.example..."
    if [ -f .env.example ]; then
        cp .env.example .env
        echo "✅ .env created - PLEASE EDIT WITH YOUR API KEYS!"
    else
        echo "❌ No .env.example found"
        echo "You'll need to create .env manually with:"
        echo "  - POSTGRES_PASSWORD"
        echo "  - NVIDIA_API_KEY (or other LLM provider)"
        echo "  - JWT_SECRET"
    fi
else
    echo "✅ .env file already exists"
fi

echo ""
echo "Step 6/6: Starting Meetily backend..."
# Stop any existing container
docker stop meetily-server 2>/dev/null || true
docker rm meetily-server 2>/dev/null || true

# Start the backend
docker-compose up -d meetily-server

if [ $? -eq 0 ]; then
    echo "✅ Backend container started"
else
    echo "❌ Failed to start backend"
    exit 1
fi

echo ""
echo "⏳ Waiting for backend to initialize (30 seconds)..."
sleep 30

echo ""
echo "📊 Container status:"
docker ps --filter "name=meetily-server" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" || echo "Container not running"

echo ""
echo "🔍 Last 20 log lines:"
docker logs meetily-server --tail 20 2>/dev/null || echo "Logs not available yet"

echo ""
echo "🌐 Testing health endpoint..."
if curl -s --max-time 5 http://localhost:8082/health 2>/dev/null; then
    echo "✅ Health check PASSED"
else
    echo "⚠️  Health check not responding yet (backend may still be starting)"
fi

echo ""
echo "========================================="
echo "✅ DEPLOYMENT COMPLETE!"
echo "========================================="
echo ""
echo "📍 Backend is running on port 8082"
echo "🌐 API URL: http://163.192.111.51:8082/api/v1"
echo "🏥 Health:  http://163.192.111.51:8082/health"
echo "📚 Docs:    http://163.192.111.51:8082/docs"
echo ""
echo "⚠️  IMPORTANT: If .env was created, edit it with your API keys:"
echo "   nano .env"
echo ""
echo "Then restart: docker-compose restart meetily-server"
echo ""
ENDSSH

if [ $? -eq 0 ]; then
    echo ""
    echo "🎉 Deployment complete!"
    echo ""
    echo "Frontend is configured to connect to:"
    echo "  http://163.192.111.51:8082/api/v1"
    echo ""
    echo "Open your browser to: http://localhost:3000"
    echo ""
else
    echo ""
    echo "❌ Deployment failed. Check output above for details."
    exit 1
fi