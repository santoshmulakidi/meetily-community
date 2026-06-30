#!/bin/bash
# Complete Meetily Backend Setup on Oracle VM
# This script: 
#   1. SSH into Oracle VM
#   2. Update docker-compose.yml to use port 8082
#   3. Start Meetily backend
#   4. Verify it's running

set -e

VM_IP="163.192.111.51"
VM_USER="ubuntu"
SSH_KEY="$HOME/.ssh/ssh-key-2026-05-28.key"

echo "🚀 Meetily Backend Setup Script"
echo "================================"
echo ""
echo "Target: $VM_USER@$VM_IP"
echo "SSH Key: $SSH_KEY"
echo "Port: 8082"
echo ""

# Check SSH connectivity first
echo "📡 Testing SSH connection..."
if ! ssh -i $SSH_KEY -o ConnectTimeout=10 -o BatchMode=yes $VM_USER@$VM_IP "exit" 2>/dev/null; then
    echo "❌ SSH connection failed!"
    echo ""
    echo "Please run manually:"
    echo "  ssh $VM_USER@$VM_IP"
    echo ""
    echo "Then run this script again once SSH is working."
    exit 1
fi

echo "✅ SSH connection successful!"
echo ""

# Run all commands on the VM
echo "🔧 Executing setup on Oracle VM..."
ssh -i $SSH_KEY $VM_USER@$VM_IP << 'ENDSSH'
#!/bin/bash
set -e

cd ~/meetily-community

echo "Step 1/4: Checking docker-compose.yml..."
if [ ! -f docker-compose.yml ]; then
    echo "❌ docker-compose.yml not found!"
    exit 1
fi

# Backup original file
cp docker-compose.yml docker-compose.yml.backup.$(date +%Y%m%d_%H%M%S)
echo "✅ Backup created"

echo ""
echo "Step 2/4: Updating port from 8080 to 8082..."
# Update the port mapping
sed -i 's/- "8080:8080"/- "8082:8080"/g' docker-compose.yml

# Verify the change
if grep -q '"8082:8080"' docker-compose.yml; then
    echo "✅ Port updated successfully"
    echo "   Changed: - \"8080:8080\" → - \"8082:8080\""
else
    echo "❌ Port update failed!"
    exit 1
fi

echo ""
echo "Step 3/4: Starting Meetily backend..."
# Stop any existing container
docker stop meetily-server 2>/dev/null || true
docker rm meetily-server 2>/dev/null || true

# Start fresh
docker-compose up -d meetily-server

if [ $? -eq 0 ]; then
    echo "✅ Backend container started"
else
    echo "❌ Failed to start backend"
    exit 1
fi

echo ""
echo "Step 4/4: Waiting for backend to initialize..."
sleep 15

echo ""
echo "📊 Container status:"
docker ps --filter "name=meetily-server" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "🔍 Last 15 log lines:"
docker logs meetily-server --tail 15

echo ""
echo "🌐 Testing health endpoint..."
if curl -s --max-time 5 http://localhost:8082/health > /dev/null 2>&1; then
    echo "✅ Health check PASSED"
    curl -s http://localhost:8082/health | head -20
else
    echo "⚠️  Health check pending (backend may still be starting)"
    echo "   Try again in 30 seconds: curl http://localhost:8082/health"
fi

echo ""
echo "========================================="
echo "✅ SETUP COMPLETE!"
echo "========================================="
echo ""
echo "📍 Backend is running on port 8082"
echo "🌐 API URL: http://$VM_IP:8082/api/v1"
echo "🏥 Health:  http://$VM_IP:8082/health"
echo "📚 Docs:    http://$VM_IP:8082/docs"
echo ""
echo "💡 Next steps:"
echo "   1. Open http://localhost:3000 in your browser"
echo "   2. Go to http://localhost:3000/login"
echo "   3. Click 'Sign up' to create your account"
echo "   4. Start using Meetily!"
echo ""
ENDSSH

if [ $? -eq 0 ]; then
    echo ""
    echo "🎉 All done! Your Meetily backend is now running on port 8082."
    echo ""
    echo "Frontend is already configured to connect to:"
    echo "  http://163.192.111.51:8082/api/v1"
    echo ""
    echo "Open your browser to: http://localhost:3000"
else
    echo ""
    echo "❌ Script failed. Please check the output above for errors."
    exit 1
fi