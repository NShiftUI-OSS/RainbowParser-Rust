#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ci_banner "job assemble"
require_env VERSION
require_cmd wasm-pack node npm python3
cd "$REPO_ROOT"
ci_overlay_versioned_source

stage_target() {
  local target="$1"
  local npm_name="$2"
  local description="$3"
  local out="build/wasm/${target}"

  log_step "wasm-pack build --target ${target}"
  rm -rf "$out"
  wasm-pack build crates/rainbow-wasm \
    --release \
    --target "$target" \
    --out-dir "$REPO_ROOT/${out}" \
    --out-name rainbow_parser_wasm

  python3 - "$out/package.json" "$npm_name" "$VERSION" "$description" <<'PY'
import json, pathlib, sys

path, name, version, description = sys.argv[1:]
package = json.loads(pathlib.Path(path).read_text())
package["name"] = name
package["version"] = version
package["description"] = description
package["license"] = "MIT"
package["repository"] = {
    "type": "git",
    "url": "https://github.com/NShiftUI-OSS/RainbowParser-Rust",
}
pathlib.Path(path).write_text(json.dumps(package, indent=2) + "\n")
PY

  [[ -f "${out}/rainbow_parser_wasm_bg.wasm" ]] || ci_die "missing wasm in ${out}"
  [[ -f "${out}/rainbow_parser_wasm.js" ]] || ci_die "missing js glue in ${out}"
  if [[ "$target" == "nodejs" ]]; then
    grep -q "exports.parse" "${out}/rainbow_parser_wasm.js" \
      || ci_die "nodejs glue is missing the CommonJS parse export"
  fi
  if [[ "$target" == "web" ]]; then
    grep -q "export function parse" "${out}/rainbow_parser_wasm.js" \
      || ci_die "web glue is missing export function parse"
    grep -q "__wbg_init as default" "${out}/rainbow_parser_wasm.js" \
      || ci_die "web glue is missing the default init export"
    grep -q "new URL('rainbow_parser_wasm_bg.wasm', import.meta.url)" "${out}/rainbow_parser_wasm.js" \
      || ci_die "web glue is missing the wasm URL"
  fi
  log_ok "staged ${out}"
}

stage_target nodejs rainbow-parser-wasm-node \
  "Rainbow DSL parser for Node.js backends (parse, format, validate, expand)."
stage_target web rainbow-parser-wasm-web \
  "Rainbow DSL parser for React and Vite (parse, format, validate, expand)."

log_step "node smoke"
node scripts/ci/release/smoke-node.mjs "$REPO_ROOT/build/wasm/nodejs/rainbow_parser_wasm.js"
log_ok "node bindings loaded the wasm package"

log_step "web smoke"
node scripts/ci/release/smoke-web.mjs "$REPO_ROOT/build/wasm/web"
log_ok "web bindings initialized the wasm package"

log_step "npm pack"
stage="$(mktemp -d)"
pack_one() {
  local dir="$1"
  local asset="$2"
  local packed
  packed="$(cd "$dir" && npm pack --pack-destination "$stage" --quiet)"
  packed="${packed##*$'\n'}"
  [[ -n "$packed" ]] || ci_die "npm pack produced no tarball in ${dir}"
  mv "${stage}/${packed}" "${stage}/${asset}"
  [[ -f "${stage}/${asset}" ]] || ci_die "missing ${asset} after npm pack"
  log_ok "packed ${asset}"
}
pack_one build/wasm/nodejs rainbow-parser-wasm-node.tgz
pack_one build/wasm/web rainbow-parser-wasm-web.tgz
[[ -f "${stage}/rainbow-parser-wasm-node.tgz" && -f "${stage}/rainbow-parser-wasm-web.tgz" ]] \
  || ci_die "refusing to publish without both the node and web packages"
rm -rf build/outputs/wasm
mkdir -p build/outputs/wasm
mv "${stage}/rainbow-parser-wasm-node.tgz" "${stage}/rainbow-parser-wasm-web.tgz" build/outputs/wasm/
log_ok "both packages ready in build/outputs/wasm"
