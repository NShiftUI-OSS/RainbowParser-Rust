#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ci_banner "job test"
require_cmd cargo wasm-pack
cd "$REPO_ROOT"
ci_overlay_versioned_source

log_step "cargo test --workspace"
cargo test --workspace
log_ok "host tests passed"

log_step "wasm-pack test --node"
( cd crates/rainbow-wasm && wasm-pack test --node )
log_ok "wasm node tests passed"
