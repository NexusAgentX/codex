# Nexus Release Runbook

Read this reference before preparing, triggering, retrying, or verifying a
Nexus release.

## Contents

1. Publishing state
2. Version selection
3. Preflight
4. Non-publishing validation
5. Publish
6. Completion criteria
7. Post-release verification
8. Failure recovery

## Publishing State

The registry bootstrap is complete. Normal tagged releases publish through npm
Trusted Publishing and GitHub OIDC. Do not request an npm password, OTP, or
token for a normal release. Treat manual bootstrap as an exceptional package
recreation operation that requires separate explicit user authorization.

The expected external contract is listed in
[fork-invariants.md](fork-invariants.md). Verify it before publishing.

## Version Selection

Read the upstream workspace version from `codex-rs/Cargo.toml`. Keep that file
unchanged. Select a monotonically increasing build number:

```text
nexus-vX.Y.Z.N -> X.Y.Z-nexus.N
```

Reset `N` to `1` after moving to a new upstream `X.Y.Z`. Increment `N` for
another Nexus release on the same upstream version. npm versions are immutable,
so never reuse a published build number.

## Preflight

Require all of the following before creating a tag:

1. The current branch is `nexus` and the worktree is clean.
2. `origin` and `upstream` match the repository identity.
3. Local `nexus` and `origin/nexus` point to the intended release commit.
4. The stable base and Nexus patch count are understood.
5. The tag version matches the workspace version.
6. The proposed tag is absent locally and remotely.
7. All seven proposed npm versions are absent from the registry.
8. Focused validation in [validation.md](validation.md) has passed.
9. `NEXUS_NPM_PUBLISH_ENABLED` is `true` and the `npm-publish` environment
   exists.
10. The intended commit contains a valid `.github/workflows/nexus-release.yml`.

Useful read-only checks:

```bash
git status --short --branch
git rev-parse HEAD
git rev-parse origin/nexus
git describe --tags --match 'rust-v*' --abbrev=0 HEAD
git tag --list 'nexus-v*' --sort=-version:refname
git ls-remote --tags origin 'refs/tags/nexus-v*'
npm view @nexus-agent-x/codex versions --json
npm view @nexus-agent-x/codex dist-tags --json
gh variable get NEXUS_NPM_PUBLISH_ENABLED --repo NexusAgentX/codex
gh api repos/NexusAgentX/codex/environments/npm-publish --jq .name
```

Treat a manual workflow dispatch and a tag push as external state changes. Do
not perform either action without explicit user authorization.

## Non-Publishing Validation

When the user explicitly asks for a packaging build without publication,
dispatch `nexus-release.yml` from `nexus`. A manual run derives `nexus.0`, does
not create a GitHub Release, and does not publish to npm.

Do not use a manual dispatch as evidence that npm OIDC works; only a tagged
release executes the registry publication job.

## Publish

After explicit authorization, create an annotated tag at the exact validated
commit and push only that tag:

```bash
tag=nexus-vX.Y.Z.N
expected_commit="$(git rev-parse HEAD)"
git tag -a "$tag" -m "Nexus Codex X.Y.Z.N" "$expected_commit"
test "$(git rev-parse "$tag^{commit}")" = "$expected_commit"
git push origin "refs/tags/$tag:refs/tags/$tag"
```

Find the resulting `nexus-release` run and watch it to completion. Do not claim
publication merely because the tag exists or npm tarballs appear on the GitHub
Release.

## Completion Criteria

Require every job below to succeed:

- release metadata
- Linux and Windows npm packaging tests
- Linux x64 and ARM64 builds
- macOS Intel and Apple Silicon builds
- Windows x64 and ARM64 builds
- complete seven-tarball assembly and validation
- GitHub Release publication
- npm OIDC publication and dist-tag verification

The GitHub Release must contain 14 assets: six canonical native archives,
seven npm tarballs, and `SHA256SUMS`. macOS and Windows executables are
unsigned community builds.

## Post-Release Verification

Verify all external surfaces after the workflow reports success:

1. The local and remote annotated tag dereference to the intended commit.
2. The GitHub Release is published, not a draft, and contains all 14 assets.
3. All 13 entries in `SHA256SUMS` match GitHub's asset digests.
4. The registry exposes the root version and all six platform versions.
5. `latest` points to the root version and all six platform dist-tags point to
   their exact platform versions.
6. A clean temporary install resolves the host platform package and reports
   the Nexus version.

Example installation smoke test:

```bash
tmp="$(mktemp -d)"
npm install --prefix "$tmp" --no-audit --no-fund \
  "@nexus-agent-x/codex@X.Y.Z-nexus.N"
"$tmp/node_modules/.bin/codex" --version
rm -rf "$tmp"
```

The expected output is `codex-cli X.Y.Z-nexus.N`.

## Failure Recovery

- If a build or assembly job fails, no release or npm root package should be
  published. Retry transient infrastructure failures. For a source or workflow
  fix, commit the fix and use the next build number; never move a pushed tag.
- If GitHub Release publication fails before npm starts, retry a transient
  failure from Actions. Use the next build number when the tagged workflow
  itself requires code changes.
- If npm fails after publishing some platform versions, the root remains
  unpublished because it is last. Rerunning the same job is safe: the workflow
  recognizes already-published exact versions, skips them, publishes missing
  versions, and verifies every dist-tag.
- If final dist-tag verification fails, inspect the registry and job log before
  changing anything. Retry registry propagation failures; do not manually
  advance `latest` without separately authorized recovery and a complete
  seven-version validation.
- Never delete or overwrite an npm version. Never retarget a pushed release
  tag. When artifact contents must change, increment `N`.
- A local tag that was never pushed may be deleted and recreated at the
  validated commit. Confirm the remote tag is absent first.
- Do not use the historical manual bootstrap path for normal recovery. Restore
  the npm Trusted Publisher mapping if OIDC configuration was removed.
