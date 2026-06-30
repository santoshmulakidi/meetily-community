#!/bin/bash
# Start Meetily Backend on Oracle VM

echo "🚀 Starting Meetily Backend on Oracle VM..."

VM_IP="163.192.111.51"
VM_USER="ubuntu"

ssh $VM_USER@$VM_IP << 'EOF'
cd ~/meetily-community

echo "📦 Checking Docker status..."
docker --version
docker-compose --version

echo ""
echo "🔧 Loading environment variables..."
if [ -f .env ]; then
    source .env
    echo "✅ Environment loaded"
else
    echo "⚠️  No .env file found, using defaults"
fi

echo ""
echo "🏗️  Starting Meetily backend..."
docker-compose up -d meetily-server

echo ""
echo "⏳ Waiting for backend to start..."
sleep 10

echo ""
echo "📊 Checking container status..."
docker ps --filter "name=meetily-server"

echo ""
echo "🔍 Checking logs..."
docker logs meetily-server --tail 20

echo ""
echo "🌐 Testing health endpoint..."
curl -s http://localhost:8080/health | head -20

echo ""
echo "✅ Meetily backend should be running!"
echo "   API: http://$VM_IP:8080/api/v1"
echo "   Health: http://$VM_IP:8080/health"
echo "   Docs: http://$VM_IP:8080/docs"
EOF

echo ""
echo "🎉 Backend startup complete!"