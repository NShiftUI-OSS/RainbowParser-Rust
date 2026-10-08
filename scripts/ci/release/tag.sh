#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job tag"
require_env TAG
require_cmd git
cd "$REPO_ROOT"

if [[ -z "${SHA:-}" ]]; then
  SHA="$(git rev-parse HEAD)"
  log_info "SHA unset; tagging HEAD ${SHA}"
fi

log_step "create tag ${TAG} at ${SHA}"
git tag "$TAG" "$SHA"
git push origin "$TAG"
log_ok "pushed ${TAG}"
