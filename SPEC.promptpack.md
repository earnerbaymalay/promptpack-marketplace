# .promptpack Specification

Version: 1.0
Status: Draft
Last Updated: 2026-11-28

---

## Overview

A `.promptpack` is a signed, self-contained JSON file that encapsulates:
- A structured prompt (system prompt, user template, few-shot examples, variables)
- Evaluation configuration (test cases, metrics, judge model)
- Cryptographic signature for integrity and authorship verification

---

## File Format

### Top-Level Structure

```json
{
  "metadata": { ... },
  "prompt": { ... },
  "eval": { ... },
  "signature": { ... }
}
```

All fields are required.

---

## Metadata Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Pack identifier: first 16 hex chars of SHA256(`author_pubkey:name`) |
| `name` | string | Yes | Human-readable pack name (kebab-case, max 64 chars) |
| `version` | string | Yes | Semantic version (MAJOR.MINOR.PATCH) |
| `author` | string | Yes | Creator's handle (GitHub username recommended) |
| `author_pubkey` | string | Yes | Ed25519 public key (32 bytes, hex-encoded, 64 chars) |
| `description` | string | Yes | One-line description (max 280 chars) |
| `tags` | string[] | Yes | Array of tags (max 10, lowercase, hyphenated) |
| `created_at` | string | Yes | ISO 8601 timestamp (UTC) |
| `updated_at` | string | Yes | ISO 8601 timestamp (UTC) |
| `license` | string | Yes | SPDX license identifier (e.g., "MIT", "Apache-2.0") |
| `homepage` | string | No | Project homepage URL |
| `repository` | string | No | Source repository URL |

**ID Generation:**
```
id = hex(sha256(author_pubkey + ":" + name))[:16]
```

---

## Prompt Object

```json
{
  "system": "string",
  "user_template": "string",
  "few_shot": FewShotExample[],
  "variables": { string: VariableSpec }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `system` | string | Yes | System prompt (instructions to the model) |
| `user_template` | string | Yes | User message template with `{variable}` placeholders |
| `few_shot` | FewShotExample[] | No | Few-shot examples (0-10) |
| `variables` | object | Yes | Map of variable name → VariableSpec |

### FewShotExample

```json
{
  "input": { "variable_name": "value" },
  "output": "expected model response"
}
```

### VariableSpec

```json
{
  "type": "string",
  "description": "string",
  "required": "boolean",
  "default": "string (optional)",
  "enum_values": "string[] (optional)"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | One of: "string", "number", "boolean", "array" |
| `description` | string | Yes | Human-readable description |
| `required` | boolean | Yes | Whether the variable must be provided |
| `default` | string | No | Default value if not provided |
| `enum_values` | string[] | No | Allowed values (for validation) |

**Template Rendering:**
- Placeholders: `{variable_name}` in `user_template`
- Replaced with user input or `default`
- Unknown placeholders left as-is
- Few-shot inputs rendered with same logic

---

## Eval Object

```json
{
  "test_cases": EvalTestCase[],
  "metrics": MetricSpec[],
  "judge_model": "string",
  "threshold": "number"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `test_cases` | EvalTestCase[] | Yes | Test cases (1-50) |
| `metrics` | MetricSpec[] | No | Custom metrics (0-10) |
| `judge_model` | string | Yes | Ollama model name for judging |
| `threshold` | number | Yes | Pass threshold (0.0-1.0) |

### EvalTestCase

```json
{
  "name": "string",
  "input": { "variable": "value" },
  "expected_output": "string (optional)",
  "criteria": "string[]"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Test case identifier |
| `input` | object | Yes | Variable values for this test |
| `expected_output` | string | No | Reference answer for judge |
| `criteria` | string[] | Yes | Evaluation criteria (1-10) |

### MetricSpec

```json
{
  "name": "string",
  "weight": "number",
  "threshold": "number"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Metric identifier |
| `weight` | number | Yes | Weight in final score (sum to 1.0) |
| `threshold` | number | Yes | Pass threshold for this metric |

---

## Signature Object

```json
{
  "algorithm": "ed25519",
  "public_key": "string",
  "signature": "string",
  "signed_at": "string"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `algorithm` | string | Yes | Must be "ed25519" |
| `public_key` | string | Yes | Signer's Ed25519 public key (hex, 64 chars) |
| `signature` | string | Yes | Ed25519 signature (hex, 128 chars) |
| `signed_at` | string | Yes | ISO 8601 timestamp (UTC) |

### Signing Process

1. **Canonicalize** the pack (exclude `signature` field)
2. **Serialize** metadata, prompt, eval as deterministic JSON (BTreeMap for maps)
3. **Hash** each section with SHA-256
4. **Concatenate** hashes: `sha256(metadata) || sha256(prompt) || sha256(eval)`
5. **Sign** concatenated hash with Ed25519 private key
6. **Encode** signature as hex

### Verification Process

1. Extract `signature` object
2. Remove `signature` field from pack JSON
3. Recompute signing data (same canonicalization)
4. Verify Ed25519 signature with `public_key`
5. Check `public_key` matches `metadata.author_pubkey`
6. Recompute pack ID, verify matches `metadata.id`

---

## Pack ID

```
pack_id = hex(sha256(author_pubkey + ":" + name))[:16]
```

- 16 hex characters (8 bytes)
- Bound to author key + pack name
- One author can publish many packs
- Collision probability negligible

---

## Evaluation Protocol

### Runner Flow

1. Load pack, verify signature
2. For each test case:
   a. Render template with case inputs + variable defaults
   b. Build messages: system → few-shot pairs → rendered template
   c. Send to Ollama (`model` param)
   d. Collect metrics: TTFT, TPOT, tokens/sec, token counts
   e. Send answer + criteria to `judge_model`
   f. Judge returns `{score: 0..1, reason: string}`
   g. Case passes if `score >= threshold`
3. Aggregate: pass rate, avg quality, avg latency

### Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `ttft_ms` | Ollama | Time to first token |
| `tpot_ms` | Ollama | Time per output token |
| `tokens_per_sec` | Ollama | `eval_count / (total_ms / 1000)` |
| `quality_score` | Judge model | 0-1 fraction of criteria met |
| `passed` | Derived | `quality_score >= threshold` |

---

## Registry Index Format

```json
{
  "version": 1,
  "updated_at": "ISO8601",
  "packs": [
    {
      "id": "string",
      "name": "string",
      "version": "string",
      "author": "string",
      "description": "string",
      "tags": "string[]",
      "download_url": "string",
      "signature": "string",
      "price_usd": "number|null",
      "created_at": "ISO8601",
      "updated_at": "ISO8601"
    }
  ]
}
```

- Hosted at `https://earnerbaymalay.github.io/promptpack-index/index.json`
- Updated via PR to `promptpack-index` repo
- `signature` field = pack's `signature.signature` (for quick verification)

---

## Security Considerations

1. **Private keys** never leave creator's machine
2. **Signatures** verified before install/eval
3. **Download tokens** for paid packs (HMAC-SHA256)
3. **Webhook signatures** verified (Stripe format)
4. **No arbitrary code execution** — only prompt templates

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-11-28 | Initial specification |

---

## References

- [Ed25519](https://ed25519.cr.yp.to/)
- [SHA-256](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf)
- [Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Stripe Checkout](https://stripe.com/docs/payments/checkout)
- [SPDX Licenses](https://spdx.org/licenses/)

---

*This specification is part of the PromptPack Marketplace project. See `CREATOR_ONBOARDING.md` for creator guide.*