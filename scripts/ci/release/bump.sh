#!/usr/bin/env bash
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers.sh"

ci_banner "job bump"
require_env VERSION
require_cmd python3
cd "$REPO_ROOT"

LIB_RS="crates/rainbow-parser/src/lib.rs"
VERSION_TESTS="crates/rainbow-parser/tests/rainbow_parser_tests.rs"
PKG_JSON="editors/vscode-rainbow/package.json"
PKG_LOCK="editors/vscode-rainbow/package-lock.json"

CURRENT="$(sed -n 's/.*pub const RAINBOW_PARSER_VERSION: &str = "\([^"]*\)".*/\1/p' "$LIB_RS")"
CURRENT="${CURRENT%%$'\n'*}"
[[ -n "$CURRENT" ]] || ci_die "could not read RAINBOW_PARSER_VERSION"
[[ "$CURRENT" != "$VERSION" ]] || ci_die "bump requested but version is already ${VERSION}"

log_step "patch working tree (no commit)"
ci_sed_inplace "s/^version = \"[^\"]+\"/version = \"${VERSION}\"/" Cargo.toml
ci_sed_inplace "s/(pub const RAINBOW_PARSER_VERSION: &str = \")[^\"]+/\1${VERSION}/" "$LIB_RS"
ci_sed_inplace "s/(assert_eq!\\(RAINBOW_PARSER_VERSION, \")[^\"]+/\1${VERSION}/" "$VERSION_TESTS"
ci_sed_inplace "s/\"version\": \"[^\"]+\"/\"version\": \"${VERSION}\"/" "$PKG_JSON"

python3 - "$VERSION" Cargo.lock "$PKG_LOCK" "$CURRENT" README.md <<'PY'
import pathlib, re, sys

version, lock_path, pkg_lock_path, current, readme_path = sys.argv[1:]

lock = pathlib.Path(lock_path)
text = lock.read_text()
for name in ("rainbow-cli", "rainbow-lsp", "rainbow-parser", "rainbow-wasm"):
    pattern = rf'(name = "{re.escape(name)}"\nversion = ")[^"]+(")'
    text, count = re.subn(pattern, rf"\g<1>{version}\2", text, count=1)
    if count != 1:
        raise SystemExit(f"failed to bump {name} in Cargo.lock ({count} matches)")
lock.write_text(text)

pkg_lock = pathlib.Path(pkg_lock_path)
lines = pkg_lock.read_text().splitlines(keepends=True)
replaced = 0
needle = f'"version": "{current}"'
replacement = f'"version": "{version}"'
for index, line in enumerate(lines[:20]):
    if needle in line:
        lines[index] = line.replace(needle, replacement, 1)
        replaced += 1
        if replaced == 2:
            break
if replaced != 2:
    raise SystemExit(f"expected 2 package-lock version fields, updated {replaced}")
pkg_lock.write_text("".join(lines))

readme = pathlib.Path(readme_path)
readme_lines = readme.read_text().splitlines(keepends=True)
hits = [index for index, line in enumerate(readme_lines) if line.strip() == current]
if len(hits) != 1:
    raise SystemExit(f"expected one standalone version line in README.md, found {len(hits)}")
newline = "\n" if readme_lines[hits[0]].endswith("\n") else ""
readme_lines[hits[0]] = version + newline
readme.write_text("".join(readme_lines))
PY

grep -F "version = \"${VERSION}\"" Cargo.toml >/dev/null \
  || ci_die "failed to patch Cargo.toml"
grep -F "pub const RAINBOW_PARSER_VERSION: &str = \"${VERSION}\"" "$LIB_RS" >/dev/null \
  || ci_die "failed to patch ${LIB_RS}"
grep -F "assert_eq!(RAINBOW_PARSER_VERSION, \"${VERSION}\")" "$VERSION_TESTS" >/dev/null \
  || ci_die "failed to patch ${VERSION_TESTS}"
grep -F "\"version\": \"${VERSION}\"" "$PKG_JSON" >/dev/null \
  || ci_die "failed to patch ${PKG_JSON}"
grep -F "name = \"rainbow-wasm\"" -A 1 Cargo.lock | grep -F "version = \"${VERSION}\"" >/dev/null \
  || ci_die "failed to patch rainbow-wasm in Cargo.lock"
log_ok "patched version files to ${VERSION}"

log_step "stage artifact files"
mkdir -p .ci-version
cp Cargo.toml .ci-version/Cargo.toml
cp Cargo.lock .ci-version/Cargo.lock
cp "$LIB_RS" .ci-version/lib.rs
cp "$VERSION_TESTS" .ci-version/rainbow_parser_tests.rs
cp "$PKG_JSON" .ci-version/package.json
cp "$PKG_LOCK" .ci-version/package-lock.json
cp README.md .ci-version/README.md
log_ok "artifact dir .ci-version ready"
