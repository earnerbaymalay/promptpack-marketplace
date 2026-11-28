# PromptPack Marketplace — Creator Onboarding Guide

Welcome to the PromptPack Marketplace! This guide walks you through creating, signing, and publishing your own `.promptpack` files.

## Quick Start

### Prerequisites

- **Rust** 1.75+ (for `create_pack` CLI)
- **Ed25519 key pair** (for signing packs)
- **Ollama** running locally (for eval)
- **Stripe account** (for paid packs, optional)

### Install the CLI

```bash
cargo install --git https://github.com/earnerbaymalay/promptpack-marketplace --locked
```

Or build from source:

```bash
git clone https://github.com/earnerbaymalay/promptpack-marketplace
cd promptpack-marketplace
cargo build --release --bin create_pack
```

---

## 1. Generate Your Signing Key

Every pack must be signed with an Ed25519 key. This proves authorship and prevents tampering.

```bash
# Generate a new key pair
promptpack gen-key > my_key.json
```

This outputs a JSON file with your keys:

```json
{
  "private_key": "base64-encoded-32-byte-secret",
  "public_key": "hex-encoded-32-byte-public-key"
}
```

**Keep `private_key` secure!** Never share it. The `public_key` goes in your pack metadata.

---

## 2. Create Your Pack

A `.promptpack` is a JSON file with this structure:

```json
{
  "metadata": {
    "id": "sha256(public_key:pack_name)[:16]",
    "name": "my-awesome-pack",
    "version": "1.0.0",
    "author": "your-github-handle",
    "author_pubkey": "your-public-key-hex",
    "description": "What this pack does",
    "tags": ["coding", "rust", "testing"],
    "created_at": "2026-01-15T10:30:00Z",
    "updated_at": "2026-01-15T10:30:00Z",
    "license": "MIT",
    "homepage": "https://github.com/you/your-pack",
    "repository": "https://github.com/you/your-pack"
  },
  "prompt": {
    "system": "You are an expert Rust developer...",
    "user_template": "Write a test for {function_name} that covers {coverage_target}",
    "few_shot": [
      {
        "input": { "function_name": "add", "coverage_target": "edge cases" },
        "output": "#[test] fn test_add_edge_cases() { ... }"
      }
    ],
    "variables": {
      "function_name": { "type": "string", "description": "Function to test", "required": true },
      "coverage_target": { "type": "string", "description": "What to cover", "required": true, "default": "happy path" }
    }
  },
  "eval": {
    "test_cases": [
      {
        "name": "basic_add",
        "input": { "function_name": "add", "coverage_target": "basic cases" },
        "expected_output": "Tests for positive/negative/zero",
        "criteria": ["Covers positive numbers", "Covers negative numbers", "Covers zero"]
      }
    ],
    "metrics": [
      { "name": "coverage", "weight": 0.5, "threshold": 0.8 },
      { "name": "correctness", "weight": 0.5, "threshold": 0.9 }
    ],
    "judge_model": "qwen2.5-coder:1.5b",
    "threshold": 0.7
  },
  "signature": {
    "algorithm": "ed25519",
    "public_key": "your-public-key-hex",
    "signature": "base64-ed25519-signature",
    "signed_at": "2026-01-15T10:30:00Z"
  }
}
```

### Using the CLI to Create a Pack

```bash
# Interactive pack creation
promptpack create-pack --output my-pack.promptpack

# Or from a directory of prompt files
promptpack create-pack --from-dir ./my-prompts --output my-pack.promptpack
```

The CLI will:
1. Prompt for metadata (name, version, description, tags)
2. Read your private key from `~/.promptpack/key.json` or `--key-file`
3. Auto-generate the pack ID from your public key + pack name
4. Sign the pack with Ed25519
5. Output a `.promptpack` file

---

## 3. Verify Your Pack

Before publishing, verify the signature:

```bash
promptpack verify-pack my-pack.promptpack
```

Expected output:
```
✓ Signature valid
✓ Pack ID matches
✓ Author key matches metadata
Pack: my-awesome-pack v1.0.0 by @yourhandle
```

---

## 4. Run Local Evaluation

Test your pack against your local Ollama models:

```bash
# Run eval on installed pack
promptpack eval my-pack --model qwen2.5-coder:1.5b --judge qwen2.5-coder:1.5b

# Or eval a pack file directly
promptpack eval ./my-pack.promptpack --model qwen2.5-coder:1.5b
```

Results include:
- **TTFT** (Time To First Token)
- **TPOT** (Time Per Output Token)
- **Tokens/sec**
- **Quality score** (0-1 from judge model)
- **Pass/fail** per test case

---

## 5. Publish to the Marketplace

### Free Packs

For free packs, host the `.promptpack` file anywhere (GitHub Releases, your website, IPFS) and submit the download URL:

```bash
# Submit via CLI (requires registry auth)
promptpack submit my-pack.promptpack --price 0

# Or manually via GitHub PR to the registry index
```

### Paid Packs (Stripe)

1. **Configure Stripe** in the Marketplace app:
   - Settings → Stripe Configuration
   - Add your **Publishable Key** (`pk_test_...` or `pk_live_...`)
   - Add your **Secret Key** (`sk_test_...` or `sk_live_...`)
   - Add your **Webhook Secret** (`whsec_...`)

2. **Set up Stripe Webhook**:
   - Go to Stripe Dashboard → Developers → Webhooks
   - Add endpoint: `https://your-domain.com/api/stripe/webhook`
   - Select events: `checkout.session.completed`
   - Copy the **Signing Secret** (`whsec_...`) to Marketplace settings

3. **Submit paid pack**:
   ```bash
   promptpack submit my-pack.promptpack --price 9.99
   ```

The marketplace will:
- Create a Stripe Checkout Session when users buy
- Verify payment via webhook
- Generate a secure download token
- Allow download only after successful payment

---

## 6. Pack Registry Index

The marketplace reads from a JSON index at `https://earnerbaymalay.github.io/promptpack-index/index.json`. Format:

```json
{
  "version": 1,
  "updated_at": "2026-01-15T10:30:00Z",
  "packs": [
    {
      "id": "a1b2c3d4e5f6",
      "name": "rust-test-generator",
      "version": "1.0.0",
      "author": "rustacean",
      "description": "Generates comprehensive Rust tests",
      "tags": ["rust", "testing", "codegen"],
      "download_url": "https://github.com/user/repo/releases/download/v1.0.0/pack.promptpack",
      "signature": "ed25519-signature-hex",
      "price_usd": 0.0,
      "created_at": "2026-01-15T10:30:00Z",
      "updated_at": "2026-01-15T10:30:00Z"
    }
  ]
}
```

To add your pack:
1. Fork the `promptpack-index` repo
2. Add your pack entry to `index.json`
3. Host your `.promptpack` file at the `download_url`
3. Open a PR

---

## 7. Best Practices

### Pack Design
- **Single responsibility**: Each pack does one thing well
- **Clear variables**: Use descriptive names, add defaults
- **Good few-shots**: 3-5 examples covering common cases
- **Realistic test cases**: Match real-world usage

### Security
- **Never commit private keys** to git
- **Use environment variables** for keys in CI
- **Rotate keys annually** (re-sign packs with new key)
- **Verify signatures** before installing any pack

### Versioning
- **Semantic versioning**: MAJOR.MINOR.PATCH
- **Update `updated_at`** on every change
- **Re-sign** after any metadata change

### Pricing
- **Free tiers**: Great for adoption
- **Paid tiers**: $5-50 for specialized packs
- **Subscription**: Not yet supported (planned)

---

## 7. Troubleshooting

### "Signature invalid"
- Check private key matches public key in metadata
- Ensure pack wasn't modified after signing
- Re-run `create_pack` to re-sign

### "Pack ID mismatch"
- Pack ID = first 16 chars of SHA256(`public_key:pack_name`)
- Don't manually edit the `id` field

### "Eval failed: model not found"
- Ensure model is pulled: `ollama pull qwen2.5-coder:1.5b`
- Check Ollama is running: `ollama ps`

### Stripe webhook not receiving events
- Check webhook URL is accessible from internet (use ngrok for local)
- Verify webhook secret matches Stripe dashboard
- Check server logs for signature verification errors

---

## 8. Resources

- **CLI Reference**: `promptpack --help`
- **Spec**: `SPEC.promptpack.md`
- **Marketplace App**: `cargo tauri dev` in `promptpack-marketplace/`
- **Registry Index**: `https://earnerbaymalay.github.io/promptpack-index/`
- **Issues**: https://github.com/earnerbaymalay/promptpack-marketplace/issues

---

## 9. Support

- **GitHub Discussions**: For questions and pack showcases
- **Discord**: #promptpack-marketplace (invite in repo)
- **Email**: earn@palexus.dev

---

*Happy packing! 📦*