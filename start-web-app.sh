#!/bin/bash
# Start Meetily Web App (Backend + Frontend)
# 
# This script starts both the Rust backend and Next.js frontend
# for local development testing.

set -e

echo "🚀 Starting Meetily Web App..."
echo ""

# Check if we're in the right directory
if [ ! -f "server/Cargo.toml" ]; then
  echo "❌ Error: Please run this script from the meetily-community root directory"
  exit 1
fi

# Check environment
echo "📋 Checking environment..."
if [ ! -f ".env" ]; then
  if [ -f ".env.example" ]; then
    echo "⚠️  .env file not found. Copying from .env.example..."
    cp .env.example .env
    echo "⚠️  Please edit .env with your API keys before running"
    exit 1
  else
    echo "❌ .env file not found. Create one with your API keys."
    exit 1
  fi
fi

# Start Rust backend
echo ""
echo "🦀 Starting Rust backend..."
cd server
cargo run &
BACKEND_PID=$!
cd ..

# Wait for backend to start
echo "⏳ Waiting for backend to start (5 seconds)..."
sleep 5

# Check if backend is running
if ! kill -0 $BACKEND_PID 2>/dev/null; then
  echo "❌ Backend failed to start. Check server logs above."
  exit 1
fi

echo "✅ Backend started (PID: $BACKEND_PID)"

# Start Next.js frontend
echo ""
echo "⚛️  Starting Next.js frontend..."
cd frontend
pnpm dev &
FRONTEND_PID=$!
cd ..

# Wait for frontend to start
echo "⏳ Waiting for frontend to start (10 seconds)..."
sleep 10

# Check if frontend is running
if ! kill -0 $FRONTEND_PID 2>/dev/null; then
  echo "❌ Frontend failed to start. Check frontend logs above."
  kill $BACKEND_PID
  exit 1
fi

echo "✅ Frontend started (PID: $FRONTEND_PID)"

echo ""
echo "========================================="
echo "🎉 Meetily Web App is running!"
echo ""
echo "📱 Frontend: http://localhost:3000"
echo "🔧 Backend:  http://localhost:8080"
echo ""
echo "Next steps:"
echo "1. Open http://localhost:3000 in your browser"
echo "2. Register a new account or login"
echo "3. Start recording your first meeting!"
echo ""
echo "To stop: Press Ctrl+C or run:"
echo "  kill $BACKEND_PID $FRONTEND_PID"
echo "========================================="
echo ""

# Wait for both processes
wait