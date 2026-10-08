#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job validate"
require_env VERSION TAG TAG_BARE
cd "$REPO_ROOT"

if [[ "$VERSION" == [vV]* ]]; then
  log_info "stripping v prefix from VERSION=${VERSION}"
  VERSION="${VERSION:1}"
fi
TAG_BARE="${TAG_BARE:-$VERSION}"
TAG="${TAG:-v${VERSION}}"

log_step "SemVer 2.0"
if [[ ! "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  ci_die "invalid SemVer 2.0: ${VERSION}"
fi
log_ok "SemVer ok (${VERSION})"

LIB_RS="crates/rainbow-parser/src/lib.rs"
[[ -f "$LIB_RS" ]] || ci_die "missing ${LIB_RS}"
CURRENT="$(sed -n 's/.*pub const RAINBOW_PARSER_VERSION: &str = "\([^"]*\)".*/\1/p' "$LIB_RS")"
CURRENT="${CURRENT%%$'\n'*}"
[[ -n "$CURRENT" ]] || ci_die "could not read RAINBOW_PARSER_VERSION"
log_info "code version=${CURRENT}"

CARGO_VERSION="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml)"
PKG_VERSION="$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' editors/vscode-rainbow/package.json)"
CARGO_VERSION="${CARGO_VERSION%%$'\n'*}"
PKG_VERSION="${PKG_VERSION%%$'\n'*}"
[[ "$CARGO_VERSION" == "$CURRENT" ]] || ci_die "Cargo.toml version ${CARGO_VERSION} != RAINBOW_PARSER_VERSION ${CURRENT}"
[[ "$PKG_VERSION" == "$CURRENT" ]] || ci_die "vscode-rainbow package.json version ${PKG_VERSION} != RAINBOW_PARSER_VERSION ${CURRENT}"
log_ok "Cargo.toml and package.json match ${CURRENT}"

ci_output sha "$(git rev-parse HEAD)"

if [[ "$CURRENT" == "$VERSION" ]]; then
  log_info "input equals code; skip bump and commit"
  ci_output bump false
else
  log_ok "input differs from code; bump and commit required"
  ci_output bump true
fi

require_cmd git gh
while IFS= read -r name; do
  log_step "remote tag ${name}"
  if git ls-remote --exit-code origin "refs/tags/${name}" >/dev/null; then
    ci_die "version already exists as tag ${name}"
  fi
  log_ok "tag ${name} is free"

  log_step "GitHub Release ${name}"
  if gh release view "$name" >/dev/null 2>&1; then
    ci_die "version already exists as GitHub Release ${name}"
  fi
  log_ok "GitHub Release ${name} is free"
done < <(ci_each_tag_name)
