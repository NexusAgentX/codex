---
name: maintain-nexus-codex
description: Maintain the NexusAgentX Codex fork and preserve its fork-specific behavior. Use whenever working on the fork's nexus branch or Nexus-owned code, including upstream stable-tag rebases, patch-stack review, main mirror updates, embedded version or native updater changes, npm packaging, six-platform builds, GitHub or npm releases, OIDC publishing, and release diagnosis.
---

# Maintain Nexus Codex

## Load The Required Context

1. Read the applicable `AGENTS.md` files for project-wide build, test, style,
   and security instructions. Keep Nexus operational policy in this skill.
2. Read [fork-invariants.md](references/fork-invariants.md) before changing
   Nexus-owned code, reviewing the patch stack, or resolving rebase conflicts.
3. Read [validation.md](references/validation.md) before validating a Nexus
   change or upstream rebase.
4. Read [release-runbook.md](references/release-runbook.md) before preparing,
   triggering, retrying, or verifying a release.
5. Inspect the worktree and remotes:

   ```bash
   git status --short --branch
   git remote -v
   ```

6. Require `origin` to point to `NexusAgentX/codex` and `upstream` to point to
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

3. Treat the scheduled `Nexus upstream check` workflow as a notification. It
   may report drift, but it must never rewrite `nexus` automatically.
4. Use `--target rust-vX.Y.Z` when the user requested a specific release.
5. Run the update only after reviewing the old base, new base, and patch count:

   ```bash
   scripts/nexus/update-upstream.sh
   ```

6. Resolve rebase conflicts without dropping the behavior documented in
   [fork-invariants.md](references/fork-invariants.md). Use the backup branch
   and `git range-diff` command printed by the script to compare the old and
   new patch stacks.
7. Follow [validation.md](references/validation.md). Ask before running the
   complete workspace test suite, as required by the repository instructions.
8. Push rewritten history only after validation:

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

1. Follow [release-runbook.md](references/release-runbook.md) from preflight
   through registry installation smoke testing.
2. Read the workspace version from `codex-rs/Cargo.toml` and inspect existing
   `nexus-v*` tags before selecting the next build number.
3. Use tags shaped as `nexus-vX.Y.Z.N`, where `X.Y.Z` exactly matches the
   workspace version and `N` is a positive, monotonically increasing build
   number.
4. Validate `.github/workflows/nexus-release.yml` and confirm the intended
   commit before creating a tag.
5. Treat manual workflow dispatch and tag pushes as external state changes.
   Do not dispatch a build or push a release tag unless the user explicitly
   authorized that action.
6. Use a manual dispatch from `nexus` for a non-publishing build check when the
   user asks to test packaging.
7. Push an annotated `nexus-v*` tag only when the user asks to publish. Watch
   the workflow to completion and verify the release assets and checksums.
8. Require the release workflow to complete Linux x64/ARM64, macOS
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
   publishing. Use `nexus-package-tests.yml` for a fast push or manual check;
   the release workflow repeats the same tests before building binaries.
6. Preserve the Nexus-aware native updater behavior: Nexus npm installations
   must check `@nexus-agent-x/codex`, compare prerelease versions correctly,
   and never redirect users to the upstream package.
7. Treat the registry bootstrap as complete. Normal releases must use npm
   Trusted Publishing for `NexusAgentX/codex`, workflow
   `nexus-release.yml`, environment `npm-publish`, and the repository variable
   `NEXUS_NPM_PUBLISH_ENABLED=true`.
8. Never fall back to a password, OTP, `NODE_AUTH_TOKEN`, or long-lived npm
   token for a normal release. Recreating a deleted package is an exceptional
   bootstrap operation and requires separate explicit user authorization.

## Report Results

State the old and new upstream bases, patch count, commit SHA, validations run,
remote mutations performed, workflow or release URL, and any remaining target
or signing gaps. Distinguish a prepared release from a published release.
