#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/nexus/update-upstream.sh [options]

Rebase the NexusAgentX patch stack onto a stable OpenAI Codex release tag.

Options:
  --branch <name>       Branch to update (default: nexus)
  --target <rust-tag>   Release tag to use (default: latest stable release)
  --dry-run             Print the planned rebase without changing refs
  -h, --help            Show this help

Environment overrides:
  NEXUS_BRANCH          Same as --branch
  NEXUS_TARGET_TAG      Same as --target
EOF
}

branch="${NEXUS_BRANCH:-nexus}"
target_tag="${NEXUS_TARGET_TAG:-}"
dry_run="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --branch)
      branch="${2:?--branch requires a value}"
      shift 2
      ;;
    --target)
      target_tag="${2:?--target requires a value}"
      shift 2
      ;;
    --dry-run)
      dry_run="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unexpected argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Run this command from inside the Codex repository." >&2
  exit 1
}
cd "$repo_root"

if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  echo "The worktree must be clean before updating the patch stack." >&2
  exit 1
fi

if ! git remote get-url upstream >/dev/null 2>&1; then
  echo "Missing upstream remote. Add https://github.com/openai/codex.git as upstream." >&2
  exit 1
fi

if ! git show-ref --verify --quiet "refs/heads/${branch}"; then
  echo "Local branch '${branch}' does not exist." >&2
  exit 1
fi

echo "+ git fetch upstream main and rust-v* tags"
git fetch --no-tags upstream \
  '+refs/heads/main:refs/remotes/upstream/main' \
  '+refs/tags/rust-v*:refs/tags/rust-v*'

old_tag="$(
  git describe --tags --match 'rust-v[0-9]*' --abbrev=0 "$branch" 2>/dev/null || true
)"
if [[ ! "$old_tag" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Could not determine the stable upstream tag beneath '${branch}'." >&2
  exit 1
fi

if [[ -z "$target_tag" ]]; then
  if command -v gh >/dev/null 2>&1; then
    target_tag="$(gh api repos/openai/codex/releases/latest --jq .tag_name)"
  else
    target_tag="$(
      git tag --list 'rust-v*' --sort=-version:refname |
        awk '/^rust-v[0-9]+\.[0-9]+\.[0-9]+$/ { print; exit }'
    )"
  fi
fi

if [[ ! "$target_tag" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Target '${target_tag}' is not a stable Codex release tag." >&2
  exit 1
fi

if ! git show-ref --verify --quiet "refs/tags/${target_tag}"; then
  echo "Tag '${target_tag}' was not fetched from upstream." >&2
  exit 1
fi

if [[ "$old_tag" == "$target_tag" ]]; then
  echo "${branch} is already based on ${target_tag}."
  exit 0
fi

patch_count="$(git rev-list --count "${old_tag}..${branch}")"
backup_branch="backup/${branch}-before-${target_tag}-$(date -u +%Y%m%dT%H%M%SZ)"

cat <<EOF
Branch:       ${branch}
Current base: ${old_tag}
Target base:  ${target_tag}
Patch count:  ${patch_count}
Backup ref:   ${backup_branch}
EOF

if [[ "$dry_run" == "true" ]]; then
  echo "+ git rebase --onto ${target_tag} ${old_tag} ${branch}"
  exit 0
fi

git branch "$backup_branch" "$branch"
git switch "$branch"

if ! git rebase --onto "$target_tag" "$old_tag"; then
  cat >&2 <<EOF
The rebase stopped on a conflict. Resolve it, then run:
  git add <resolved-files>
  git rebase --continue

To return to the pre-update state, run:
  git rebase --abort
  git reset --hard ${backup_branch}
EOF
  exit 1
fi

cat <<EOF
Updated ${branch} from ${old_tag} to ${target_tag}.

Next steps:
  1. Run the tests for the crates affected by the patch stack.
  2. Review: git range-diff ${old_tag}..${backup_branch} ${target_tag}..${branch}
  3. Publish: git push --force-with-lease origin ${branch}
EOF
