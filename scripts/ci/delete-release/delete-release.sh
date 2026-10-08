#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job delete-release"
require_env TAG TAG_BARE GH_TOKEN GITHUB_REPOSITORY
require_cmd gh

deleted=false
while IFS= read -r name; do
  log_step "GitHub Release ${name}"
  if ! gh release view "$name" --repo "$GITHUB_REPOSITORY" >/dev/null 2>&1; then
    log_info "no GitHub Release named ${name}"
    continue
  fi

  assets="$(gh release view "$name" --repo "$GITHUB_REPOSITORY" --json assets --jq '.assets[].name')"
  log_info "assets before delete:"
  if [[ -n "$assets" ]]; then
    printf '%s\n' "$assets" | while IFS= read -r asset; do
      log_info "  ${asset}"
    done
  else
    log_info "  <none>"
  fi
  for required in rainbow-parser-wasm-node.tgz rainbow-parser-wasm-web.tgz; do
    if printf '%s\n' "$assets" | grep -Fxq "$required"; then
      log_info "will remove ${required} with the release"
    else
      log_warn "release ${name} has no ${required}; deleting the release still removes every asset that is there"
    fi
  done

  gh release delete "$name" --yes --repo "$GITHUB_REPOSITORY"
  if gh release view "$name" --repo "$GITHUB_REPOSITORY" >/dev/null 2>&1; then
    ci_die "release ${name} still exists; node and web packages were not both removed"
  fi
  log_ok "deleted GitHub Release ${name} (node and web packages together)"
  deleted=true
done < <(ci_each_tag_name)

ci_output deleted "$deleted"
if [[ "$deleted" == true ]]; then
  log_ok "release cleanup done"
else
  log_info "no GitHub Release matched ${TAG} or ${TAG_BARE}; tag job will check git tags"
fi
