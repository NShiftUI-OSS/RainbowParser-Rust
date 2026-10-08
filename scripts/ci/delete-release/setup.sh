#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job setup"
require_env ALLOWED_ACTOR GITHUB_ACTOR TAG_INPUT

if [[ "$GITHUB_ACTOR" != "$ALLOWED_ACTOR" ]]; then
  ci_die "only ${ALLOWED_ACTOR} can run this workflow (actor=${GITHUB_ACTOR})"
fi
log_ok "actor ${GITHUB_ACTOR} allowed"

ci_parse_tag_input "$TAG_INPUT"
log_info "input=${TAG_INPUT}"
log_info "tag=${TAG}"
log_info "tag_bare=${TAG_BARE}"
ci_output tag "$TAG"
ci_output tag_bare "$TAG_BARE"
log_ok "setup outputs written"
