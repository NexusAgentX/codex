# Nexus Validation

Read this reference before validating a Nexus change or an upstream rebase.

## Select Checks By Changed Surface

Inspect both the upstream delta and the Nexus patch stack. Run focused tests
for every affected crate; do not assume a conflict-free rebase is behaviorally
identical.

For Rust changes, run from `codex-rs`:

```bash
just test -p <affected-crate>
just fix -p <affected-crate>
just fmt
```

Follow the root `AGENTS.md` ordering: run tests before the final `fix` and
`fmt`, and do not rerun tests afterward. Ask before running the complete
workspace suite.

For Nexus npm packaging changes, run from the repository root:

```bash
python -m unittest discover -s codex-cli/scripts -p 'test_*.py'
```

Confirm the `Nexus package tests` workflow passes on both Linux and Windows
when `codex-cli/**` or its packaging workflow changes.

## Release-Tag Version Snapshots

Upstream development snapshots commonly expect version `0.0.0`, while a
stable release tag gives Cargo a real `X.Y.Z` workspace version. If TUI
snapshot failures differ only in the displayed version, rerun the focused
suite with:

```bash
CODEX_CLI_VERSION_OVERRIDE=0.0.0 just test -p codex-tui
```

Do not accept snapshot changes solely to replace `0.0.0` with the current
release version. The release workflow separately injects the derived Nexus
version into production binaries.

## Isolate Suspected Upstream Flakes

If a test times out after an upstream rebase, run it alone and compare its
implementation across the old and new upstream bases and the Nexus patch
stack. Only classify it as upstream or fixture timing after confirming Nexus
did not change the relevant path. Report any excluded or flaky test explicitly;
never silently turn a failure into a pass.

## Final Hygiene

Before pushing a rebased branch or release tag, require:

```bash
git diff --check
git status --short --branch
```

Remove generated `*.snap.new` files and unintended lockfile version expansion.
Require a clean worktree, local `nexus` at the intended commit, and local and
remote branch state understood before using `--force-with-lease`.
