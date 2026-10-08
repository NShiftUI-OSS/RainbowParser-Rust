#!/usr/bin/env bash
# Shared by test/assemble/commit (not a GitHub job).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_overlay_versioned_source() {
  log_step "overlay versioned-source"
  local src=".ci-version"
  if [[ ! -d "$src" ]]; then
    log_info "no versioned-source overlay; using checkout as-is"
    return 0
  fi
  local name
  for name in Cargo.toml Cargo.lock lib.rs rainbow_parser_tests.rs package.json package-lock.json README.md; do
    [[ -f "${src}/${name}" ]] || ci_die "missing ${src}/${name}"
  done
  cp "${src}/Cargo.toml" Cargo.toml
  cp "${src}/Cargo.lock" Cargo.lock
  cp "${src}/lib.rs" crates/rainbow-parser/src/lib.rs
  cp "${src}/rainbow_parser_tests.rs" crates/rainbow-parser/tests/rainbow_parser_tests.rs
  cp "${src}/package.json" editors/vscode-rainbow/package.json
  cp "${src}/package-lock.json" editors/vscode-rainbow/package-lock.json
  cp "${src}/README.md" README.md
  log_ok "overlaid version files onto checkout"
}
