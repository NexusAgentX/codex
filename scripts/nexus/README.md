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

## Publish a build

The initial release workflow builds the complete Linux x86_64 musl package.
It can be tested from the GitHub Actions UI without publishing a release.

After updating and testing `nexus`, create a monotonically increasing build
tag whose first three components match `codex-rs/Cargo.toml`:

```bash
git tag -a nexus-v0.144.3.1 -m "Nexus Codex 0.144.3.1"
git push origin nexus-v0.144.3.1
```

Only a pushed `nexus-v*` tag publishes a GitHub Release. Pushing the `nexus`
branch alone does not start the release workflow.

## npm package validation

The npm scope is `@nexus-agent-x`, and the CLI package is
`@nexus-agent-x/codex`. A manual `nexus-release` run builds Linux x86_64 npm
tarballs without publishing them. A release tag also attaches those tarballs
to the GitHub Release as validation artifacts.

Do not publish the root package to npmjs.com until every platform version it
references has been built and tested. The initial workflow provides only the
Linux x86_64 platform version. Publishing requires a separate, explicitly
authorized npm step. Before the first registry release, adapt and test the
native TUI update checker so Nexus npm installations resolve both release
metadata and update commands through `@nexus-agent-x/codex`; the inherited
implementation still targets the upstream GitHub releases and npm package.
