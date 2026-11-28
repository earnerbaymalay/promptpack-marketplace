# PromptPack Marketplace

Local-first desktop app for buying, verifying, and evaluating **prompt packs**: signed bundles of prompts you can benchmark against your own local models before you trust them.

Built with Tauri 2 (Rust core) and a Vite + TypeScript frontend. Evaluation runs through a local Ollama server; nothing leaves your machine.

## What's here

| Path | Purpose |
|------|---------|
| `src-tauri/src/pack.rs` | `.promptpack` format: create, verify (Ed25519), extract; watch a packs folder |
| `src-tauri/src/eval.rs` | Runs a pack's eval cases on a local model; a judge model scores each answer against its criteria; TTFT/TPOT/tok/s from Ollama |
| `src-tauri/src/registry.rs` | Fetch the registry `index.json`, download and submit packs |
| `src-tauri/src/store.rs` | App config, eval history, installed packs |
| `src/` | Frontend (Vite + TypeScript) |
| `packs/` | 10 signed seed packs (code review, security analysis, threat modeling, Rust perf, ...) |
| `public/index.json` | Registry index served from GitHub Pages |

## `.promptpack` format

A JSON document with `metadata` (id, name, version, author, tags, license), the prompts, eval cases, and a `signature` block: an Ed25519 signature over SHA-256 of the metadata, prompt and eval sections. `verify_pack` rejects a pack if the signature doesn't verify, if `metadata.author_pubkey` isn't the signing key, or if `metadata.id` isn't the first 16 hex chars of `sha256("<pubkey>:<name>")`.

### Signing packs (creators)

```bash
cd src-tauri
cargo run --bin sign-pack -- keygen                 # once; key saved to %APPDATA%\promptpack\creator.key
cargo run --bin sign-pack -- sign ../packs/*.promptpack
cargo run --bin sign-pack -- verify ../packs/*.promptpack
cargo run --bin sign-pack -- index --out ../public/index.json --base-url https://earnerbaymalay.github.io/promptpack-index/packs ../packs/*.promptpack
```

Then copy `packs/` and `public/index.json` into the `promptpack-index` repo, which GitHub Pages serves. Back up `creator.key`; never commit it.

## Build

Prerequisites: Rust 1.75+, Node.js 20+, the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/), and [Ollama](https://ollama.com) for evals.

```bash
./build.sh        # install deps + build (build.cmd on Windows)
./dev.sh          # dev mode: Vite + Tauri (dev.cmd on Windows)
```

Ollama host, default model and temperature come from the shared **palexus-brain** config (`palexus-brain config show`; override with e.g. `PALEXUS_BRAIN_INFERENCE__DEFAULT_MODEL=qwen3.5:2b`).

This repo builds inside the `ai-cli-work` workspace: it depends on `../shared/ollama-client` and `../shared/palexus-brain-schema`.

### Evaluate a pack from the terminal

```bash
cd src-tauri
cargo run --bin eval-pack -- ../packs/code-review-expert.promptpack [--model qwen3.5:2b]
```

Results are appended to `<palexus-brain data dir>/promptpack/evals.jsonl` (shown in the app's eval history).

## Status

Phase 1 of the master plan. Done: pack spec, signing tool and verification, eval runner, registry index, 10 signed seed packs, UI scaffold. Still to do: Stripe Checkout, signed delivery of paid packs, creator onboarding docs.

## License

MIT OR Apache-2.0
