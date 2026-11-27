# PromptPack Marketplace

Local-first desktop app for buying, verifying, and evaluating **prompt packs**: signed bundles of prompts you can benchmark against your own local models before you trust them.

Built with Tauri 2 (Rust core) and a Vite + TypeScript frontend. Evaluation runs through a local Ollama server; nothing leaves your machine.

## What's here

| Path | Purpose |
|------|---------|
| `src-tauri/src/pack.rs` | `.promptpack` format: create, verify (Ed25519), extract; watch a packs folder |
| `src-tauri/src/eval.rs` | Local eval runner against Ollama (latency / token metrics) |
| `src-tauri/src/registry.rs` | Fetch the registry `index.json`, download and submit packs |
| `src-tauri/src/store.rs` | App config, eval history, installed packs |
| `src/` | Frontend (Vite + TypeScript) |
| `packs/` | 10 seed packs (code review, security analysis, threat modeling, Rust perf, ...); not yet signed |
| `public/index.json` | Registry index served from GitHub Pages |

## `.promptpack` format

A JSON document with `metadata` (id, name, version, author, tags, license), the prompts, and a `signature` block (Ed25519 public key + signature over the content). `verify_pack` rejects a pack whose signature does not verify, or whose `metadata.id` is not the first 16 hex chars of the signing public key.

## Build

Prerequisites: Rust 1.75+, Node.js 20+, the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/), and [Ollama](https://ollama.com) for evals.

```bash
./build.sh        # install deps + build (build.cmd on Windows)
./dev.sh          # dev mode: Vite + Tauri (dev.cmd on Windows)
```

Copy `.env.example` to `.env` to point at a different Ollama host, model, or registry URL.

## Status

Phase 1 of the master plan. Done: pack spec and verification, eval runner, registry index, seed pack content, UI scaffold. Still to do: sign the seed packs (they carry placeholder signatures), Stripe Checkout, signed delivery of paid packs, creator onboarding docs.

## License

MIT OR Apache-2.0
