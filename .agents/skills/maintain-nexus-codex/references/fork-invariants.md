# Nexus Fork Invariants

Read this reference before changing Nexus-owned behavior, reviewing the patch
stack, or resolving an upstream rebase conflict.

## Repository Identity

- `origin` must be `NexusAgentX/codex`.
- `upstream` must be `openai/codex`.
- `main` is an upstream mirror and contains no Nexus patches.
- `nexus` is the default distribution branch. It is a linear Nexus patch stack
  based on a stable `rust-vX.Y.Z` tag, never a moving `main` commit.
- Discover the current base, patch count, and release tag from Git. Do not
  hardcode a version from this reference.

Use these commands to reconstruct current state:

```bash
git status --short --branch
git remote -v
git describe --tags --match 'rust-v*' --abbrev=0 nexus
base="$(git describe --tags --match 'rust-v*' --abbrev=0 nexus)"
git rev-list --count "${base}..nexus"
git tag --list 'nexus-v*' --sort=-version:refname
```

## Behavior That Must Survive Rebases

| Area | Invariant | Primary owners |
| --- | --- | --- |
| Version identity | Keep the workspace version equal to upstream. Derive `X.Y.Z-nexus.N` from `nexus-vX.Y.Z.N` and embed it in release binaries. | `codex-rs/cli/src/main.rs`, `codex-rs/tui/src/version.rs`, `codex-rs/cli/tests/version.rs` |
| Distribution identity | The public wrapper is `@nexus-agent-x/codex` and provides the `codex` executable. It must not silently install or redirect to `@openai/codex`. | `codex-cli/package.json`, `codex-cli/bin/codex.js`, `codex-cli/README.nexus.md` |
| Native updater | Detect Nexus npm installations, query `@nexus-agent-x/codex`, compare prerelease versions correctly, and keep update actions on the Nexus package. | `codex-rs/install-context`, `codex-rs/cli/src/doctor/updates.rs`, `codex-rs/tui/src/update_*`, `codex-rs/tui/src/updates*` |
| npm layout | Build one root wrapper plus Linux x64/ARM64, macOS x64/ARM64, and Windows x64/ARM64 variants. Every tarball contains the same package name at a distinct exact version; the root uses npm aliases. | `codex-cli/scripts/build_npm_package.py`, `codex-cli/scripts/validate_nexus_npm_release.py`, their tests |
| Release integrity | Require all six native builds before assembling the seven npm tarballs. Attach canonical packages, exact npm artifacts, and `SHA256SUMS` to the GitHub Release. | `.github/workflows/nexus-release.yml` |
| Registry ordering | Publish platform versions serially under platform dist-tags, then publish the root under `latest`. Never expose a root package whose exact platform dependency is absent. | `.github/workflows/nexus-release.yml` |
| Upstream notification | The scheduled check reports stable-tag drift but never rewrites `nexus`. | `.github/workflows/nexus-upstream-check.yml`, `scripts/nexus/update-upstream.sh` |

## External Publishing Contract

The initial npm package bootstrap has already completed. Normal publication
uses this external state:

- npm package: `@nexus-agent-x/codex`
- GitHub organization/repository: `NexusAgentX/codex`
- Trusted Publisher workflow: `nexus-release.yml`
- GitHub environment: `npm-publish`
- Allowed Trusted Publisher action: `npm publish`
- Repository variable: `NEXUS_NPM_PUBLISH_ENABLED=true`
- No stored npm token or `NODE_AUTH_TOKEN`

Verify external state before a release instead of assuming it still exists:

```bash
gh variable get NEXUS_NPM_PUBLISH_ENABLED --repo NexusAgentX/codex
gh api repos/NexusAgentX/codex/environments/npm-publish --jq .name
npm view @nexus-agent-x/codex dist-tags --json
```

A prior successful `Publish npm package` OIDC job proves that the npm Trusted
Publisher mapping worked at that time. If the mapping is later removed, stop
and restore it in npm rather than introducing a token.
