# vscode-rainbow

Language support for the **Rainbow** DSL in **VS Code** and **Cursor**.

Uses the `rainbow-lsp` binary from this repository (`crates/rainbow-lsp`), which
wraps `rainbow-parser` (kept in parity with the Swift and Kotlin parsers).

## Features

- Syntax highlighting (TextMate) with Rainbow palette:
  - pins: `use` purple · name SwiftUI lilac · SemVer pink
  - plugins lilac · events yellow · triggers red (params inherit owner)
  - string orange · number green · bool fuchsia · null rose · identifier cyan
- Diagnostics from the Rainbow parser
- Format Document / format on save (canonical printer):
  - 1 scalar param stays inline; **≥2 params**, a breaking object, or multiline `@LANG` break one-per-line (no trailing comma)
  - objects with **≥2 fields** break; the closing `)` aligns with the parameter or parent field; a one-field object stays compact
  - plugins and events without a children block format as constructors (`Spinner()`)
  - a children block with no parameters formats as `Name {`. `Name() {` is reported as `rainbow.parser.emptyConstructorBlock`
  - trailing comma after the last parameter or object field is a parse error
  - blocks always expand `{` / `}` across lines (including empty)
- Naming checks on `.rbw`:
  - plugin / event / trigger / `use` names must be UpperCamelCase
  - same name cannot be both a plugin (root/body) and an event (under `On*`)
  - plugin and event calls without a children block require `()`
  - `Name() {` is `rainbow.parser.emptyConstructorBlock`; write `Name {` when there are no parameters
  - an `On*` trigger with no children is an error
- Template placeholders `#{Name}` (use / node / value); backends expand via
  `rainbow expand --map vars.json`
- Embedded `@LANG(...)` parameter values (raw payload, not a Rainbow string):

  | Tag | Tag color (dark / light) | Body highlight | Validate / format |
  |-----|--------------------------|----------------|-------------------|
  | `@JSON` | `#FBBF24` / `#D97706` | `json` | yes / pretty |
  | `@YAML` | `#34D399` / `#059669` | `yaml` | yes / pretty |
  | `@XML` | `#FB7185` / `#E11D48` | `xml` | well-formed / pretty |
  | `@HTML` | `#F97316` / `#EA580C` | `html` | tokenizer errors / conservative |
  | `@MARKDOWN` | `#38BDF8` / `#0284C7` | `markdown` | fences + light normalize |

  Whole-blob template: `@JSON(#{Name})`. Format comes from `rainbow-lsp`.
  In the editor, **bundled** language services (JSON / YAML / HTMLHint /
  markdownlint / XML parser) provide diagnostics, hover, and completions inside
  `@LANG(...)` bodies. Toggle with `rainbow.embedded.bundledLanguageServices`
  (default on). CLI `rainbow validate` still uses the Rust validators.
- Optional **name registry** (`rainbow.registryPath` or `*.registry.json`):
  - fails if the same name is listed as both plugin and event (no document tree)
  - completions include registered plugin/event names
- Document symbols
- Hover
- Completions (`use` snippet + names from the open document / registry)
- Folding ranges

Token colors live in `themes/rainbow-tokens-{dark,light}.json` and are applied on
activation (and when the color theme flips). They merge into
`editor.tokenColorCustomizations` as rules named `Rainbow: …`.

## End-user install (recommended)

With the public Homebrew tap:

```bash
brew tap NShiftUI-OSS/rainbowparser
brew trust NShiftUI-OSS/rainbowparser
brew install rainbow
rainbow install cursor    # or: vscode | both
```

`rainbow install` uses the VSIX packed by Homebrew (`share/rainbow/`). It also writes
`"rainbow.lsp.path"` and `"[rainbow].editor.defaultFormatter"`
(`tcc-nshiftui.vscode-rainbow`) so Format Document / format on save work after
**Developer: Reload Window**.

The packaged VSIX **must** include runtime dependency `vscode-languageclient`
(do **not** use `vsce --no-dependencies`, and do not ignore all of `node_modules`
in `.vscodeignore`). Without that, the editor shows Rainbow highlighting but no
LSP diagnostics or formatter.

### Language icon

One universal transparent mark (`icons/rainbow.png`) is used for both light and dark
themes (`package.json` points `light` and `dark` to the same file).

```bash
npm run icons:install
```

Then **Developer: Reload Window** and check the explorer icon on a `.rbw` file.

## Setup (extension development)

From the repo root:

```bash
cargo build -p rainbow-lsp
cd editors/vscode-rainbow
npm install
npm run compile
# optional: npm run package   # produces .vsix
```

In VS Code / Cursor:

1. `Extensions: Install from Location…` → select `editors/vscode-rainbow`
   **or** open this folder and press F5 (Extension Development Host).
2. Open a `.rbw` file.

### Binary path

By default the extension looks for:

1. Setting `rainbow.lsp.path`
2. `target/release/rainbow-lsp` or `target/debug/rainbow-lsp` next to the repo
3. `rainbow-lsp` on `PATH`

```json
{
  "rainbow.lsp.path": "/absolute/path/to/rainbow-lsp"
}
```

## CI

Homebrew packages the universal VSIX from source. The GitHub Release ships the Node and Web WASM packages.

## Parity

When Rainbow syntax changes in the mobile parsers, update
`crates/rainbow-parser` in this repo first, then rebuild `rainbow-lsp`. See
`spec/grammar.md` and shared `fixtures/`.
