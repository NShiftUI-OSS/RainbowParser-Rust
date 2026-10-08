#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job setup"
require_env ALLOWED_ACTOR GITHUB_ACTOR VERSION_INPUT BRANCH_INPUT

if [[ "$GITHUB_ACTOR" != "$ALLOWED_ACTOR" ]]; then
  ci_die "only ${ALLOWED_ACTOR} can run this workflow (actor=${GITHUB_ACTOR})"
fi
log_ok "actor ${GITHUB_ACTOR} allowed"

BRANCH="$(echo "$BRANCH_INPUT" | tr -d '[:space:]')"
[[ -n "$BRANCH" ]] || ci_die "branch input is empty"

ci_parse_tag_input "$VERSION_INPUT"
VERSION="$TAG_BARE"
log_info "input=${VERSION_INPUT}"
log_info "version=${VERSION}"
log_info "branch=${BRANCH}"
log_info "tag=${TAG}"

ci_output version "$VERSION"
ci_output branch "$BRANCH"
ci_output tag "$TAG"
ci_output tag_bare "$TAG_BARE"
log_ok "setup outputs written"
