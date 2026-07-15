# @nexus-agent-x/codex

This package is the unofficial NexusAgentX distribution of OpenAI Codex CLI.
It is built from the [`NexusAgentX/codex`](https://github.com/NexusAgentX/codex)
fork and carries the fork's additional patches.

Install it globally with:

```bash
npm install -g @nexus-agent-x/codex
```

When replacing an existing official npm installation, uninstall it first;
both packages provide the same `codex` executable:

```bash
npm uninstall -g @openai/codex
npm install -g @nexus-agent-x/codex
```

The release pipeline builds and validates native packages for:

- Linux x64 and ARM64
- macOS Intel and Apple Silicon
- Windows x64 and ARM64

macOS and Windows executables are unsigned community builds.

The upstream project is [`openai/codex`](https://github.com/openai/codex).
This distribution is licensed under Apache-2.0 and is not an official OpenAI
release.
