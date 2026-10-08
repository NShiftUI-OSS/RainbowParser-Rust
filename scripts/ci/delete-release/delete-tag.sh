#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job delete-tag"
require_env TAG TAG_BARE
require_cmd git
cd "$REPO_ROOT"

deleted=false
while IFS= read -r name; do
  log_step "remote tag ${name}"
  if git ls-remote --exit-code origin "refs/tags/${name}" >/dev/null; then
    git push origin ":refs/tags/${name}"
    log_ok "deleted remote tag ${name}"
    deleted=true
  else
    log_info "no remote tag ${name}"
  fi
done < <(ci_each_tag_name)

if [[ "$deleted" != true && "${RELEASE_DELETED:-false}" != true ]]; then
  ci_die "nothing to delete: no GitHub Release and no git tag named ${TAG} or ${TAG_BARE}"
fi
if [[ "$deleted" != true ]]; then
  log_info "no git tag left; GitHub Release was already removed"
fi
log_ok "tag cleanup done"
