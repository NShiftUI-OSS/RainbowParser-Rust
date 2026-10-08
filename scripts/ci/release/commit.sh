#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ci_banner "job commit"
require_env VERSION BRANCH
require_cmd git
cd "$REPO_ROOT"
ci_overlay_versioned_source

log_step "commit version bump"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Cargo.toml Cargo.lock \
  crates/rainbow-parser/src/lib.rs \
  crates/rainbow-parser/tests/rainbow_parser_tests.rs \
  editors/vscode-rainbow/package.json \
  editors/vscode-rainbow/package-lock.json \
  README.md
if git diff --cached --quiet; then
  ci_die "nothing to commit; overlay did not change version files"
fi
git commit -m "chore: bump version to ${VERSION}"
log_ok "committed $(git rev-parse --short HEAD)"

log_step "push ${BRANCH}"
git push origin "HEAD:${BRANCH}"
SHA="$(git rev-parse HEAD)"
ci_output sha "$SHA"
log_ok "pushed ${SHA} to ${BRANCH}"
