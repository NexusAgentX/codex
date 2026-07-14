# NexusAgentX Codex maintenance

This directory contains the maintenance tooling for the unofficial
`NexusAgentX/codex` fork.

## Branches

- `main` mirrors `openai/codex` and must not contain NexusAgentX changes.
- `nexus` is the fork's default branch. It starts at a stable `rust-vX.Y.Z`
  tag and contains the NexusAgentX patch stack as small, reviewable commits.

## Update the upstream base

The OpenAI release tags can be sibling commits, so do not merge one release
tag into another. Replay only the NexusAgentX commits onto the new tag:

```bash
scripts/nexus/update-upstream.sh --dry-run
scripts/nexus/update-upstream.sh
```

Pass `--target rust-vX.Y.Z` to select a specific stable release. The script
creates a backup branch before it changes `nexus` and prints a `range-diff`
command for reviewing the result.

The `Nexus upstream check` workflow compares this base with OpenAI's latest
stable GitHub Release every Monday at 02:17 UTC. It fails with a preview
command in the job summary when an update is available; it never rewrites the
branch. The workflow can also be run manually from GitHub Actions.

## Build and publish a GitHub Release

The release workflow builds native packages for Linux x64/ARM64, macOS
Intel/Apple Silicon, and Windows x64/ARM64. It can be tested from the GitHub
Actions UI without publishing a release or writing to npm.

After updating and testing `nexus`, create a monotonically increasing build
tag whose first three components match `codex-rs/Cargo.toml`:

```bash
git tag -a nexus-v0.144.3.1 -m "Nexus Codex 0.144.3.1"
git push origin nexus-v0.144.3.1
```

Only a pushed `nexus-v*` tag publishes a GitHub Release. Pushing the `nexus`
branch alone does not start the release workflow.

The upstream version remains unchanged in `codex-rs/Cargo.toml`. A tag such as
`nexus-v0.144.3.1` derives the Nexus version `0.144.3-nexus.1` for the binary,
package metadata, and npm tarballs. Increment the final tag component for
additional Nexus builds on the same upstream version, and reset it to `1` when
moving to a new upstream version. Manual validation runs use `nexus.0` and are
never published.

## npm packages

The npm scope is `@nexus-agent-x`, and the CLI package is
`@nexus-agent-x/codex`. A manual `nexus-release` run builds seven tarballs
without publishing them: six platform versions and one root wrapper. A release
tag attaches those exact tarballs to the GitHub Release.

Every tarball is a version of the same npm package. Platform versions use
suffixes such as `0.144.3-nexus.1-linux-x64`; the root version references them
through npm aliases. Platform versions must be published serially before the
root version advances `latest`.

Changes under `codex-cli/` trigger the lightweight `Nexus package tests`
workflow on Linux and Windows. Use it to validate packaging changes without
starting the six-platform Rust build; the release workflow repeats those tests
before producing binaries.

The workflow's npm job is disabled unless the repository variable
`NEXUS_NPM_PUBLISH_ENABLED` is exactly `true`. Do not enable it before the
package exists and npm Trusted Publishing is configured.

### First registry release

The first release requires an explicitly authorized manual bootstrap because
npm cannot configure a trusted publisher for a package that does not exist.
After a tagged GitHub Release succeeds, download and validate its npm assets:

```bash
release_tag=nexus-v0.144.3.1
version=0.144.3-nexus.1
npm_dir="$(mktemp -d)"
gh release download "$release_tag" \
  --repo NexusAgentX/codex \
  --pattern 'codex-npm-*.tgz' \
  --dir "$npm_dir"
python codex-cli/scripts/validate_nexus_npm_release.py \
  --version "$version" \
  --tarball-dir "$npm_dir"
```

With the `nexus-agent-x` npm login and its second factor available, publish
each platform version under its own dist-tag, then publish the root last:

```bash
for platform in \
  linux-x64 linux-arm64 \
  darwin-x64 darwin-arm64 \
  win32-x64 win32-arm64
do
  npm publish "$npm_dir/codex-npm-${platform}-${version}.tgz" \
    --access public \
    --tag "$platform"
done
npm publish "$npm_dir/codex-npm-${version}.tgz" \
  --access public \
  --tag latest
```

Never put an npm password, one-time code, or long-lived token in this
repository or a workflow file.

### Trusted Publishing

After the bootstrap, follow npm's
[Trusted Publishing documentation](https://docs.npmjs.com/trusted-publishers/)
and configure the package with:

- GitHub organization/user: `NexusAgentX`
- Repository: `codex`
- Workflow filename: `nexus-release.yml`
- Environment: `npm-publish`
- Allowed actions: `npm publish`

Create and protect the matching GitHub environment, then enable OIDC
publication:

```bash
gh variable set NEXUS_NPM_PUBLISH_ENABLED \
  --repo NexusAgentX/codex \
  --body true
```

The tag workflow will then validate the complete set again, publish all six
platform versions serially, publish the root under `latest`, and verify every
dist-tag. It uses no `NODE_AUTH_TOKEN` or stored npm secret.

The native updater is Nexus-aware: npm installations check
`@nexus-agent-x/codex`, preserve prerelease ordering, and do not redirect to
the upstream package.
