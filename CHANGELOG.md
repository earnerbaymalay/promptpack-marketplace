# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versioning: [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Tauri 2 + Rust + Vite/TypeScript app scaffold (Nocta theme UI)
- `.promptpack` spec with Ed25519 signature verification
- Local eval runner against Ollama (latency, tokens/sec)
- Registry index (`public/index.json`) and 10 seed packs
- Tauri build script, capabilities, and icon; CI workflow; Dependabot config

### Changed
- `tauri.conf.json` migrated to Tauri v2 syntax

### Removed
- Build output (`src-tauri/target/`) and `node_modules/` from version control
