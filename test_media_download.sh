#!/bin/bash

# Test script for Bird.com media download
# Usage: ./test_media_download.sh

set -e

echo "🧪 Bird.com Media Download Test"
echo "================================"

# Load environment variables from .env file
if [ -f ".env" ]; then
    echo "📄 Loading .env file..."
    export $(grep -v '^#' .env | xargs)
fi

# Check required environment variables
if [ -z "$BIRD_API_KEY" ]; then
    echo "❌ BIRD_API_KEY not found in environment"
    exit 1
fi

if [ -z "$BIRD_WORKSPACE_ID" ]; then
    echo "❌ BIRD_WORKSPACE_ID not found in environment"
    exit 1
fi

# Test media URL from the webhook
MEDIA_URL="https://media.api.bird.com/workspaces/ccaf31d9-5e11-4497-8ace-df07972f3f96/messages/aa00af8e-412a-4047-b308-0ef5ac53f457/media/8bf2e22c-b088-4537-abf2-fe1cdc6dd729"

echo "🔑 API Key: ${BIRD_API_KEY:0:10}..."
echo "🏢 Workspace ID: $BIRD_WORKSPACE_ID"
echo "📸 Media URL: $MEDIA_URL"
echo ""

# Create test directory
mkdir -p test_output

echo "📥 Testing download with Bearer token + Workspace ID header..."

# Test 1: Bearer token + Workspace ID header
echo "Test 1: Bearer + X-Workspace-Id"
curl -s -L -w "\n📊 HTTP Status: %{http_code}\n" \
  -H "Authorization: Bearer $BIRD_API_KEY" \
  -H "X-Workspace-Id: $BIRD_WORKSPACE_ID" \
  -o test_output/test1_bearer_workspace.jpg \
  "$MEDIA_URL" 2>/dev/null

if [ -f test_output/test1_bearer_workspace.jpg ] && [ -s test_output/test1_bearer_workspace.jpg ]; then
    echo "✅ Test 1 SUCCESS: $(ls -lh test_output/test1_bearer_workspace.jpg | awk '{print $5}') bytes"
    file test_output/test1_bearer_workspace.jpg
else
    echo "❌ Test 1 FAILED"
fi

echo ""

# Test 2: AccessKey token + Workspace ID header
echo "Test 2: AccessKey + X-Workspace-Id"
curl -s -L -w "\n📊 HTTP Status: %{http_code}\n" \
  -H "Authorization: AccessKey $BIRD_API_KEY" \
  -H "X-Workspace-Id: $BIRD_WORKSPACE_ID" \
  -o test_output/test2_accesskey_workspace.jpg \
  "$MEDIA_URL" 2>/dev/null

if [ -f test_output/test2_accesskey_workspace.jpg ] && [ -s test_output/test2_accesskey_workspace.jpg ]; then
    echo "✅ Test 2 SUCCESS: $(ls -lh test_output/test2_accesskey_workspace.jpg | awk '{print $5}') bytes"
    file test_output/test2_accesskey_workspace.jpg
else
    echo "❌ Test 2 FAILED"
fi

echo ""

# Test 3: Bearer token only
echo "Test 3: Bearer only"
curl -s -L -w "\n📊 HTTP Status: %{http_code}\n" \
  -H "Authorization: Bearer $BIRD_API_KEY" \
  -o test_output/test3_bearer_only.jpg \
  "$MEDIA_URL" 2>/dev/null

if [ -f test_output/test3_bearer_only.jpg ] && [ -s test_output/test3_bearer_only.jpg ]; then
    echo "✅ Test 3 SUCCESS: $(ls -lh test_output/test3_bearer_only.jpg | awk '{print $5}') bytes"
    file test_output/test3_bearer_only.jpg
else
    echo "❌ Test 3 FAILED"
fi

echo ""

# Test 4: No authentication
echo "Test 4: No auth"
curl -s -L -w "\n📊 HTTP Status: %{http_code}\n" \
  -o test_output/test4_no_auth.jpg \
  "$MEDIA_URL" 2>/dev/null

if [ -f test_output/test4_no_auth.jpg ] && [ -s test_output/test4_no_auth.jpg ]; then
    echo "✅ Test 4 SUCCESS: $(ls -lh test_output/test4_no_auth.jpg | awk '{print $5}') bytes"
    file test_output/test4_no_auth.jpg
else
    echo "❌ Test 4 FAILED"
fi

echo ""
echo "📁 Test files created in test_output/ directory"
ls -la test_output/

echo ""
echo "🎯 Summary:"
echo "- If any test succeeds, we know the correct authentication method"
echo "- Check the HTTP status codes above"
echo "- Successful downloads will have file type information"

# Clean up
echo ""
read -p "🧹 Clean up test files? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf test_output
    echo "✅ Test files cleaned up"
fi
