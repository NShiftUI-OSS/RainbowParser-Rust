#!/usr/bin/env bash
# Package and install the extension with the universal language icon.
#
# Usage:
#   ./scripts/use-icons.sh --install

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON="$ROOT/icons/rainbow.png"

if [[ ! -f "$ICON" ]]; then
  echo "Missing $ICON"
  exit 1
fi

if [[ "${1:-}" != "--install" ]]; then
  echo "Universal icon: $ICON"
  echo "Run with --install to package and install into Cursor/VS Code."
  exit 0
fi

cd "$ROOT"
npm run package
VSIX="$(ls -1 "$ROOT"/vscode-rainbow-*.vsix | head -1)"
echo "Installing $VSIX …"

if command -v cursor >/dev/null 2>&1; then
  cursor --install-extension "$VSIX" --force
fi
if command -v code >/dev/null 2>&1; then
  code --install-extension "$VSIX" --force
fi

echo ""
echo "Done. Developer: Reload Window, then open a .rbw file."
