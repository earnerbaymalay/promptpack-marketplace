#!/usr/bin/env bash
# PromptPack Marketplace Dev Script

set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
NC='\033[0m'

echo -e "${BLUE}[INFO]${NC} Starting PromptPack Marketplace in development mode..."
echo -e "${BLUE}[INFO]${NC} This will start Vite dev server and Tauri app"
echo

npm run tauri dev