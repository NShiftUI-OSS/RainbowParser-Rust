# RainbowParser Rust

`rainbowparser-rust` is the backend/frontend-agnostic syntax package for the
Rainbow DSL. It parses Rainbow source into a generic AST, prints the AST back to
canonical Rainbow text, and returns structured diagnostics.

Core parse/format in **this crate** accept templates with `#{Name}` placeholders
and embedded `@LANG(...)` parameter payloads (`JSON` / `YAML` / `XML` / `HTML` /
`MARKDOWN`). That template/expand surface is the **global backend API**.

Mobile parsers (iOS, Android) consume **concrete** Rainbow only. They do not
implement `#{Name}` substitution.

`rainbow validate` (and the LSP) check naming/positions and embedded bodies;
`rainbow expand --map vars.json` is the backend substitution step (including
`@JSON(#{blob})`) before shipping concrete Rainbow. Use `validate --concrete` /
`expand --validate` to forbid leftover holes. Runtime dual plugin/event
registration still fatals in iOS `NShiftRegisteredVersionCatalog`.

## Version

Current beta:

```text
0.1.0-beta.1
```

The runtime version is exposed as:

```rust
rainbow_parser::RAINBOW_PARSER_VERSION
```

## Workspace

```text
crates/
  rainbow-parser/  # core lexer, parser, AST, diagnostics, formatter, JSON output
  rainbow-cli/     # CLI binary: rainbow
  rainbow-lsp/     # Language Server (VS Code / Cursor / other LSP clients)
  rainbow-wasm/    # WASM bindings for the Node backend and React
editors/
  vscode-rainbow/  # VS Code + Cursor extension (TypeScript client)
fixtures/          # golden fixtures shared with other implementations
spec/              # grammar and AST JSON schema
```

## Cross-implementation parity

This crate is the **global** Rainbow syntax source of truth for backends and
tooling. Keep it aligned with:

- iOS Swift: `mobile/iOS/packages/rainbowparser`
- (future) Android / other runtimes

When syntax changes (e.g. `use Name@1.0.0`, `@`, SemVer tokens, `.enum`), update
**this** parser + `spec/grammar.md` + `fixtures/`, then mirror the **concrete**
subset into Swift/Android. Do **not** mirror `#{Name}` / `expand` into mobile
libraries — those stay in this crate (see `docs/PARITY.md`).

## Requirements

- Rust 1.86 or newer
- Cargo
- Node 20+ (only when building the VS Code / Cursor extension from source)

## Source files

Rainbow source files use the **`.rbw`** extension (editor language id remains `rainbow`).

## End-user install (Homebrew + editor)

Private tap (SSH access to the GitLab group required):

```bash
brew tap tcc-nshiftui/tap git@gitlab.com:tcc-nshiftui/global/homebrew-tap.git
brew install rainbow
rainbow install cursor    # or: vscode | both | (interactive)
```

`brew install rainbow` installs `rainbow`, `rainbow-lsp`, and a prebuilt `.vsix`
under `$(brew --prefix rainbow)/share/rainbow/`.

`rainbow install` detects the `cursor` / `code` CLIs, installs the VSIX, and
writes editor settings so Format Document works after reload:
`"rainbow.lsp.path"` (Homebrew/`PATH` `rainbow-lsp`) and
`"[rainbow].editor.defaultFormatter"` (`tcc-nshiftui.vscode-rainbow`).
Use `--no-settings` to skip the settings write. Then **Developer: Reload Window**.

Optional: if the VSIX is missing locally, set a GitLab token to download from the
Package Registry:

```bash
export GITLAB_TOKEN="<read_api personal access token>"
rainbow install cursor
```

## Commands

```bash
cargo test --workspace
cargo build -p rainbow-lsp
cargo run -p rainbow-cli -- parse --json fixtures/valid/home.rbw
cargo run -p rainbow-cli -- format fixtures/valid/home.rbw
cargo run -p rainbow-cli -- validate --json fixtures/valid/home.rbw
cargo run -p rainbow-cli -- expand --map fixtures/templates/example.map.json fixtures/templates/example.rbw
cargo run -p rainbow-cli -- validate-registry fixtures/registry/valid.registry.json
cargo run -p rainbow-cli -- install --list
cargo run -p rainbow-cli -- doctor
```

CLI UX: colored ✓/✗ feedback on TTY (`NO_COLOR` / `FORCE_COLOR` respected),
`rainbow doctor` for environment checks, and step-based `rainbow install`.


### VS Code / Cursor extension (development)

```bash
cargo build -p rainbow-lsp
cd editors/vscode-rainbow
npm install
npm run compile
npm run package   # optional .vsix
```

Then install the folder as an extension (or press F5). See
`editors/vscode-rainbow/README.md`.

The CLI also reads from stdin:

```bash
echo 'Button(title: "Entrar")' | cargo run -p rainbow-cli -- parse --json -
```

## Go Integration

The first integration path is the CLI JSON contract. This avoids `cgo`, native
linking and memory ownership issues while the grammar is still evolving.

```go
cmd := exec.Command("rainbow", "parse", "--json", "screen.rbw")
out, err := cmd.Output()
if err != nil {
    // Exit code 2 means syntax diagnostics were returned in stdout.
}
```

Successful parse responses look like:

```json
{
  "ok": true,
  "document": {
    "uses": [],
    "nodes": []
  },
  "diagnostics": []
}
```

Syntax errors return exit code `2` and:

```json
{
  "ok": false,
  "document": null,
  "diagnostics": []
}
```

## Supported Syntax

```text
Document   = Node*
Node       = Identifier Arguments? Block?
Arguments  = "(" ParameterList? ")"
Parameter  = Identifier ":" Value
Block      = "{" Node* "}"
Value      = String | Number | Bool | EnumIdentifier | Array | Object | Null
EnumIdentifier = "." Identifier
```

Example:

```rainbow
Screen(name: "Home") {
  Button(title: "Entrar", variant: .primary) {
    OnTap {
      Navigate(to: "Dashboard")
    }
  }
}
```

Supported values:

```rainbow
Node(
  text: "value",
  int: -1,
  double: 1.25,
  enabled: true,
  disabled: false,
  variant: .primary,
  items: ["a", 1, null],
  meta: (id: "home", "dash-key": false),
  emptyArray: [],
  emptyObject: ()
)
```

## Public Rust API

```rust
use rainbow_parser::{decode, encode};

let document = decode(r#"Button(title: "Entrar")"#)?;
let formatted = encode(&document);
```

## Release

Actions → **Release** (branch + SemVer with or without `v`). Delete: **Delete release** (SemVer with or without `v`). Only `ArthurPorto-PucMinas`.

If the SemVer input already matches `RAINBOW_PARSER_VERSION` and the tag/Release does not exist yet, bump and commit are skipped and the WASM artifacts publish at the current version. If the tag or the GitHub Release already exists, validate fails.

The bump updates the workspace `version`, `RAINBOW_PARSER_VERSION`, the version assertion in `rainbow_parser_tests`, `editors/vscode-rainbow` `package.json` / `package-lock.json`, and the beta line in this README.

The YAML files in `.github/workflows/` only orchestrate. Each job's logic lives in `scripts/ci/release/` and `scripts/ci/delete-release/`. Colors and failures with context: `scripts/helpers.sh`. To reproduce a job locally:

```bash
ALLOWED_ACTOR=ArthurPorto-PucMinas GITHUB_ACTOR=ArthurPorto-PucMinas \
  VERSION_INPUT=0.1.0-beta.1 BRANCH_INPUT=main \
  bash scripts/ci/release/setup.sh
```

One **Release** workflow builds the Node package and the React package in the same job. If either build fails, nothing is published. **Delete release** deletes that GitHub Release, so both packages go away together. Node installs `rainbow-parser-wasm-node.tgz`. React/Vite installs `rainbow-parser-wasm-web.tgz`. The JSON envelopes match `rainbow <command> --json`. See `docs/WASM.md`.

## Roadmap

- Keep golden fixtures aligned with iOS and Android implementations.
- Add optional C ABI once the CLI JSON contract stabilizes.

