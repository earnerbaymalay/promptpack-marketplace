# Contributing

Thank you for contributing! This project follows the **Nocta philosophy**: local-first, zero cloud deps, dogfood everything, reuse aggressively, document as you go, no timeframes.

## Getting Started

### Prerequisites

- **Rust**: 1.75+ ([rustup](https://rustup.rs/))
- **Node.js**: 20+ ([nvm](https://github.com/nvm-sh/nvm) / [fnm](https://github.com/Schniz/fnm))
- **Git**: 2.40+

### Setup

```bash
# Clone
git clone https://github.com/earnerbaymalay/promptpack-marketplace
cd promptpack-marketplace

# Install dependencies
cd frontend && npm install

# Verify build
./build.sh build
```

## Workflow

### 1. Create Issue First

Before writing code, [open an issue](https://github.com/earnerbaymalay/promptpack-marketplace/issues/new/choose) describing:
- The problem
- Proposed solution
- Alternatives considered

### 2. Branch Naming

```
<type>/<issue-number>-<short-description>

Examples:
feat/123-add-user-auth
fix/456-memory-leak
docs/789-update-readme
refactor/101-simplify-config
```

### 3. Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]

Types:
  feat     - New feature
  fix      - Bug fix
  docs     - Documentation only
  style    - Formatting, no code change
  refactor - Code restructure, no behavior change
  perf     - Performance improvement
  test     - Add or fix tests
  chore    - Maintenance, no code change
  build    - Build system changes
  ci       - CI/CD changes

Examples:
feat(auth): add OAuth2 login
fix(memory): resolve leak in buffer pool
docs(readme): add installation section
refactor(config): simplify schema validation
```

### 4. Pull Requests

- Target `main` branch
- Fill out PR template completely
- Link related issues: `Fixes #123`
- Ensure CI passes
- Request review from maintainers

### 5. Code Review

Review criteria:
- [ ] Solves the stated problem
- [ ] Follows code style (fmt, clippy, prettier)
- [ ] Tests pass (cargo test, npm test)
- [ ] No new warnings
- [ ] Documentation updated
- [ ] CHANGELOG.md entry added (for user-facing changes)

## Code Standards

### Rust

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace -- -D warnings

# Test
cargo test --workspace
```

### TypeScript/Frontend

```bash
# Format
cd frontend && npx prettier --write .

# Lint
cd frontend && npm run lint

# Type check
cd frontend && npx tsc --noEmit
```

## Testing

```bash
# All tests
./build.sh test

# Rust only
cargo test --workspace

# Frontend only
cd frontend && npm test
```

## Release Process

Maintainers only:

1. Update version in `Cargo.toml` and `package.json`
2. Update `CHANGELOG.md`
3. Create tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. GitHub Actions creates release automatically

## Code of Conduct

This project follows the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

## Questions?

Open a [Discussion](https://github.com/earnerbaymalay/promptpack-marketplace/discussions) or [Issue](https://github.com/earnerbaymalay/promptpack-marketplace/issues).