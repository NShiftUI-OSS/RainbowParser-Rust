# Rainbow parser parity

## Rule

`global/rainbowparser-rust` is the cross-platform syntax authority.
Platform parsers (iOS Swift today, Android later) must not invent grammar that
is missing here.

## Checklist after any syntax change

1. Update `spec/grammar.md` and `spec/ast.schema.json`.
2. Add/adjust `fixtures/valid` and `fixtures/invalid`.
3. Implement in `crates/rainbow-parser` (lexer → parser → printer → JSON).
4. `cargo test --workspace`.
5. Rebuild `rainbow-lsp` (`cargo build -p rainbow-lsp`).
6. Mirror the same behavior in `mobile/iOS/packages/rainbowparser`.
7. Note the change in the release / harness docs of both repos.

## Current shared features (keep in sync)

| Feature | Notes |
|---------|--------|
| Nodes / params / blocks | `()` config, `{}` composition |
| Values | string, int, double, bool, null, enum identifier (`.name`), array, object, `@LANG(...)` tagged |
| Enum identifier values | Leading `.` required in source (`.primary`); AST value omits the dot |
| `use Name@1.2.3` | Concrete top-level SemVer pins (version required) |
| `use #{Name}` | Template pin; expand value must be `Name@SemVer` |
| `//` comments | Line comments through end of line |
| `#{Name}` placeholders | Template holes; expand via CLI/API before concrete ship |
| `@LANG(...)` embedded | Param values only; `JSON`/`YAML`/`XML`/`HTML`/`MARKDOWN`; whole-blob `#{Name}` |
| `@` token | `use` pins and `@LANG(` tagged values |
| Version lexeme | `1.0.0` (2+ dots); `1.25` remains double |
| Canonical printer | Uses before nodes; blank between siblings; preserves leading `//`; `≥2` params (or multiline `@LANG`) break; block braces expand; objects use `(…)`; `@LANG` bodies re-indented |

## Plugin / event namespace (tooling)

Plugin and event names share one namespace. Kinds come from **document position**:

| Position | Kind |
|----------|------|
| Document root / child of plugin body (not `On*`) | plugin |
| Child of `On*` (not `On*`) | event |
| `OnTap`, `OnSuccess`, … | trigger (neither) |

- `rainbow validate` / LSP (`analyze_document_name_kinds`):
  - error if a plugin / event / trigger / `use` name is not UpperCamelCase ASCII letters
  - error if the same name is used in both plugin and event positions
- Optional registry JSON for backends without a document tree: `{ "plugins": [...], "events": [...] }` via `rainbow validate-registry` / `rainbow.registryPath`.
- Runtime authority: iOS `NShiftRegisteredVersionCatalog` (fatal on dual registration).
- Diagnostic code: `rainbow.semantic.nameKindConflict`

Do not bake kinds into the grammar/AST.

## LSP

`crates/rainbow-lsp` always depends on this parser. Do not reimplement grammar
inside `editors/vscode-rainbow`.

Custom request `rainbow/embeddedRegions` lists `@LANG(...)` bodies for the
extension’s **bundled** language services (JSON/YAML/HTML/Markdown/XML). Editor
IDE features for those bodies live in the extension; CLI validation stays in Rust.
