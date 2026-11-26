#!/usr/bin/env bash
# PromptPack Marketplace Build Script (Unix/Linux/macOS/WSL)

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

log_info "PromptPack Marketplace Build Script"
echo

log_info "Checking prerequisites..."
command -v node >/dev/null 2>&1 || { log_error "Node.js not found"; exit 1; }
command -v npm >/dev/null 2>&1 || { log_error "npm not found"; exit 1; }
command -v cargo >/dev/null 2>&1 || { log_error "Rust/Cargo not found"; exit 1; }
log_success "Prerequisites satisfied"
echo

log_info "Installing frontend dependencies..."
npm install
log_success "Frontend dependencies installed"
echo

log_info "Building Rust backend (debug)..."
cargo build --manifest-path src-tauri/Cargo.toml
log_success "Rust backend built"
echo

log_info "Building frontend..."
npm run build
log_success "Frontend built"
echo

log_info "Building Tauri application..."
npm run tauri build
log_success "Tauri application built successfully"
echo

log_success "All builds completed!"
log_info "Run 'npm run tauri dev' for development mode"