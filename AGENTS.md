# RainbowParser-Rust — Agent Harness

Crate Rust de sintaxe do DSL Rainbow **e** API global do backend. Versão `0.1.0-beta.1`.

Autoridade da gramática: `spec/grammar.md`, `spec/ast.schema.json`, `fixtures/`, `docs/PARITY.md`.

## Papel

```text
template .rbw  →  expand (este crate)  →  Rainbow concreto  →  iOS / Android
```

- **Este repo:** parse/format de templates, `expand`, validate, registry, CLI, LSP, WASM.
- **iOS / Android:** só Rainbow concreto. Sem `#{Name}`, sem expand.
- **Node:** pacote `rainbow-parser-wasm-node` (expand no backend).
- **React:** pacote `rainbow-parser-wasm-web` (`await init()`, depois parse/format/validate no editor).

Não espelhar furos de template nas libs mobile. Detalhes: `docs/PARITY.md`.

## Comandos

```bash
cargo test --workspace
cargo run -p rainbow-cli -- expand --map fixtures/templates/example.map.json fixtures/templates/example.rbw
```

Release e delete seguem o esquema do NShiftUI-Android (Actions manuais, scripts em `scripts/ci/`). Contrato Node/React: `docs/WASM.md`.
