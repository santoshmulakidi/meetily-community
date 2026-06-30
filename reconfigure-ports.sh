#!/bin/bash
# Reconfigure Open WebUI to use port 8082 instead of 8080
# This frees up port 8080 for Meetily backend

echo "🔄 Reconfiguring Open WebUI to port 8082..."

# SSH into Oracle VM
VM_IP="163.192.111.51"
VM_USER="ubuntu"

echo "📡 Connecting to $VM_USER@$VM_IP..."

# Find and update Open WebUI docker container or compose file
ssh $VM_USER@$VM_IP << 'EOF'
echo "Checking for Open WebUI..."

# Check if running via docker-compose
if [ -f ~/docker-compose.yml ] || [ -f ~/open-webui/docker-compose.yml ]; then
    echo "Found docker-compose setup"
    
    # Find the compose file
    COMPOSE_FILE=$(find ~ -name "docker-compose.yml" -o -name "compose.yml" 2>/dev/null | grep -i webui | head -1)
    
    if [ -z "$COMPOSE_FILE" ]; then
        COMPOSE_FILE=$(find ~ -name "docker-compose.yml" 2>/dev/null | head -1)
    fi
    
    if [ -n "$COMPOSE_FILE" ]; then
        echo "Updating: $COMPOSE_FILE"
        
        # Backup first
        cp "$COMPOSE_FILE" "$COMPOSE_FILE.backup"
        
        # Update port mapping from 8080:8080 to 8082:8080
        sed -i 's/"8080:8080"/"8082:8080"/g' "$COMPOSE_FILE"
        sed -i 's/- "8080:8080"/- "8082:8080"/g' "$COMPOSE_FILE"
        
        echo "✅ Port updated in compose file"
        
        # Restart the container
        cd $(dirname "$COMPOSE_FILE")
        docker-compose down
        docker-compose up -d
        
        echo "✅ Open WebUI restarted on port 8082"
    else
        echo "❌ Could not find docker-compose file"
    fi
else
    # Check if running as standalone docker container
    CONTAINER_ID=$(docker ps --filter "ancestor=ghcr.io/open-webui/open-webui" --format "{{.ID}}")
    
    if [ -n "$CONTAINER_ID" ]; then
        echo "Found standalone container: $CONTAINER_ID"
        
        # Stop old container
        docker stop $CONTAINER_ID
        docker rm $CONTAINER_ID
        
        # Start new container on port 8082
        docker run -d \
            --name open-webui \
            -p 8082:8080 \
            -v open-webui:/app/backend/data \
            --restart always \
            ghcr.io/open-webui/open-webui:main
            
        echo "✅ Open WebUI restarted on port 8082"
    else
        echo "❌ Open WebUI container not found"
    fi
fi

echo ""
echo "Verifying ports..."
docker ps --format "table {{.Names}}\t{{.Ports}}"

echo ""
echo "✅ Configuration complete!"
echo "   - Open WebUI: http://$VM_IP:8082"
echo "   - Meetily backend: http://$VM_IP:8080 (ready to start)"
EOF

echo ""
echo "🎉 Done! Open WebUI should now be on port 8082"
echo "   Meetily backend can now use port 8080"