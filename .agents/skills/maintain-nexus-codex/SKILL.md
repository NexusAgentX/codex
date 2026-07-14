---
name: maintain-nexus-codex
description: Maintain the NexusAgentX Codex fork in this repository by preserving its branch model, rebasing the Nexus patch stack onto stable OpenAI rust-v releases, validating custom changes, and preparing or publishing nexus-v builds. Use when asked to sync or upgrade the fork, inspect upstream drift, add Nexus-specific changes, update the mirror branch, prepare a custom Codex build, tag a Nexus release, or diagnose the nexus-release workflow.
---

# Maintain Nexus Codex

## Establish Context

1. Read the applicable `AGENTS.md` files before editing.
2. Read `scripts/nexus/README.md` before changing branch history or preparing a
   release.
3. Inspect the worktree and remotes:

   ```bash
   git status --short --branch
   git remote -v
   ```

4. Require `origin` to point to `NexusAgentX/codex` and `upstream` to point to
   `openai/codex`. Stop and report unexpected remotes rather than rewriting
   them silently.

## Preserve The Branch Model

- Keep `main` free of Nexus changes; it is the upstream mirror.
- Keep the custom patch stack on `nexus`, the fork's default branch.
- Base `nexus` on a stable `rust-vX.Y.Z` tag, not on a moving `main` commit.
- Keep each custom behavior in a small, reviewable commit with focused tests.
- Do not merge one OpenAI release tag into another. Release tags can be sibling
  commits, so replay only the Nexus commits with the repository update script.
- Never discard unrelated worktree changes or rewrite a shared branch without
  first checking its remote state.

## Update The Nexus Patch Stack

1. Require a clean `nexus` worktree. If user changes are present, stop and ask
   how to preserve them.
2. Preview the latest stable update:

   ```bash
   scripts/nexus/update-upstream.sh --dry-run
   ```

3. Use `--target rust-vX.Y.Z` when the user requested a specific release.
4. Run the update only after reviewing the old base, new base, and patch count:

   ```bash
   scripts/nexus/update-upstream.sh
   ```

5. Resolve rebase conflicts without dropping Nexus behavior. Use the backup
   branch and `git range-diff` command printed by the script to compare the old
   and new patch stacks.
6. Run `just fmt` from `codex-rs`, then run focused tests for every changed
   crate with `just test -p <crate>`. Ask before running the complete workspace
   test suite, as required by the repository instructions.
7. Push rewritten history only after validation:

   ```bash
   git push --force-with-lease origin nexus
   ```

   Never use an unconditional force push.

## Maintain The Main Mirror

Update `main` separately from the Nexus patch stack. Verify that the update is
fast-forward before pushing `upstream/main` to `origin/main`. Warn that a push
to `main` starts the fork's upstream CI workflows. Never merge `nexus` into
`main`.

## Change Nexus Behavior

1. Work on `nexus` or a short-lived branch based on it.
2. Keep generic fixes suitable for OpenAI upstream separable from
   Nexus-specific policy or product changes.
3. Avoid broad formatting, generated-file, or dependency churn unless the
   change requires it.
4. Follow the repository's formatting, focused-test, snapshot, and generated
   schema requirements before committing.
5. Report which commits are Nexus-only and which could be submitted upstream.

## Prepare And Publish A Release

1. Read the workspace version from `codex-rs/Cargo.toml` and inspect existing
   `nexus-v*` tags before selecting the next build number.
2. Use tags shaped as `nexus-vX.Y.Z.N`, where `X.Y.Z` exactly matches the
   workspace version and `N` is a positive, monotonically increasing build
   number.
3. Validate `.github/workflows/nexus-release.yml` and confirm the intended
   commit before creating a tag.
4. Treat manual workflow dispatch and tag pushes as external state changes.
   Do not dispatch a build or push a release tag unless the user explicitly
   authorized that action.
5. Use a manual dispatch from `nexus` for a non-publishing build check when the
   user asks to test packaging.
6. Push an annotated `nexus-v*` tag only when the user asks to publish. Watch
   the workflow to completion and verify the release assets and checksums.
7. Require the release workflow to complete Linux x64/ARM64, macOS
   Intel/Apple Silicon, and Windows x64/ARM64 before claiming a release is
   ready. macOS and Windows executables are unsigned community builds.

## Prepare npm Artifacts

1. Use `@nexus-agent-x/codex` as the root package and derive platform aliases
   from that name.
2. Keep `codex-rs/Cargo.toml` at the upstream version. Derive
   `<upstream>-nexus.<build>` from the final numeric component of the
   `nexus-v<upstream>.<build>` tag, and use it consistently for the embedded
   CLI version, package metadata, and npm packages.
3. Treat npm tarballs attached to a GitHub Release as validation artifacts,
   not proof that a registry publish occurred.
4. Do not publish the root package while any of its six referenced platform
   versions is missing. Publish platform versions serially under their
   platform dist-tags, then publish the root version under `latest`.
5. Run the focused Python package tests on Linux and Windows, validate the
   complete seven-tarball set, and inspect `npm pack` metadata before
   publishing.
6. Preserve the Nexus-aware native updater behavior: Nexus npm installations
   must check `@nexus-agent-x/codex`, compare prerelease versions correctly,
   and never redirect users to the upstream package.
7. Require explicit user authorization before the first `npm publish`. The
   first registry release must be bootstrapped with the user's npm login and
   second factor because Trusted Publishing cannot be configured before the
   package exists.
8. After the bootstrap release, configure npm Trusted Publishing for
   `NexusAgentX/codex`, workflow `nexus-release.yml`, and environment
   `npm-publish`, allowing only `npm publish`. Keep
   `NEXUS_NPM_PUBLISH_ENABLED` unset until that setup is complete; then use the
   OIDC job instead of storing a long-lived token.

## Report Results

State the old and new upstream bases, patch count, commit SHA, validations run,
remote mutations performed, workflow or release URL, and any remaining target
or signing gaps. Distinguish a prepared release from a published release.
