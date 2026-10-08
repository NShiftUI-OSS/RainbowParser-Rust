#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job release"
require_env TAG VERSION BRANCH GH_TOKEN
require_cmd gh find
cd "$REPO_ROOT"

DIST="${WASM_DIR:-dist}"
log_step "find artifacts in ${DIST}"
[[ -d "$DIST" ]] || ci_die "missing artifact download dir ${DIST}"
NODE_TGZ="$(find "$DIST" -name 'rainbow-parser-wasm-node.tgz' -print -quit || true)"
WEB_TGZ="$(find "$DIST" -name 'rainbow-parser-wasm-web.tgz' -print -quit || true)"
[[ -n "$NODE_TGZ" ]] || ci_die "no rainbow-parser-wasm-node.tgz in ${DIST}"
[[ -n "$WEB_TGZ" ]] || ci_die "no rainbow-parser-wasm-web.tgz in ${DIST}"
log_ok "attaching ${NODE_TGZ}"
log_ok "attaching ${WEB_TGZ}"

log_step "gh release create ${TAG}"
gh release create "$TAG" "$NODE_TGZ" "$WEB_TGZ" \
  --repo "${GITHUB_REPOSITORY:?}" \
  --title "$TAG" \
  --notes "$(cat <<EOF
WASM ${VERSION} built from branch \`${BRANCH}\`.

- \`rainbow-parser-wasm-node.tgz\` — Node.js backend (\`require\`)
- \`rainbow-parser-wasm-web.tgz\` — React / Vite (\`await init()\` then \`parse\`)

Both packages are one release. Delete release removes them together.
Integration: \`docs/WASM.md\`.
EOF
)"

log_step "confirm both assets on ${TAG}"
assets="$(gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json assets --jq '.assets[].name')"
missing=0
for required in rainbow-parser-wasm-node.tgz rainbow-parser-wasm-web.tgz; do
  if printf '%s\n' "$assets" | grep -Fxq "$required"; then
    log_ok "published ${required}"
  else
    log_error "missing ${required}"
    missing=1
  fi
done
if [[ "$missing" == 1 ]]; then
  log_step "roll back partial release ${TAG}"
  gh release delete "$TAG" --yes --repo "$GITHUB_REPOSITORY"
  ci_die "release ${TAG} did not contain both packages, so it was deleted"
fi
log_ok "GitHub Release ${TAG} published with node and web packages"
