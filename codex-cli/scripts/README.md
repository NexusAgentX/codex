# npm releases

Use the staging helper in the repo root to generate npm tarballs for a release. For
example, to stage the CLI, responses proxy, and SDK packages for version `0.6.0`:

```bash
./scripts/stage_npm_packages.py \
  --release-version 0.6.0 \
  --package codex \
  --package codex-responses-api-proxy \
  --package codex-sdk
```

This downloads the required native package archive artifacts, hydrates `vendor/` for
each package, and writes tarballs to `dist/npm/`.

When `--package codex` is provided, the staging helper builds the lightweight
`@nexus-agent-x/codex` meta package plus all platform-native
`@nexus-agent-x/codex` variants that are later published under
platform-specific versions.

Direct `build_npm_package.py` invocations are still useful for package-specific
debugging, but native packages expect `--vendor-src` to point at a prehydrated
`vendor/` tree. Full cross-platform release packaging should use
`scripts/stage_npm_packages.py`. The Nexus single-target validation workflow
invokes the package builder directly until all target builds are available.
