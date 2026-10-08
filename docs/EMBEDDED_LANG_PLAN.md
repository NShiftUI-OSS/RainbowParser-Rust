# Plano: valores embutidos `@Lang(...)` em parâmetros Rainbow

Status: **implementado** (Rust toolchain + extensão TextMate/cores + parity Swift parse/AST).  
Canvas de cores: `canvases/rainbow-embedded-lang-colors.canvas.tsx` (workspace Cursor).  
Última consolidação: 2026-07-19.

---

## 1. Objetivo

Permitir que parâmetros Rainbow carreguem payloads de linguagens de marcação/dados já existentes, com:

1. Sintaxe tipada `@LANG(…payload cru…)` (não exigir string Rainbow).
2. Validação de erro **completa** da linguagem dentro dos `(...)`.
3. Highlight + cores distintas por `@Lang` na extensão.
4. Formatação correta do envelope Rainbow **e** do body embutido.
5. Interpolação só via blob inteiro: `@LANG(#{name})` — **somente no crate Rust** (`rainbow expand`). iOS/Android não implementam essa substituição; recebem o body já concreto.

Mobile continua recebendo Rainbow **concreto** (pós-expand + validate).

---

## 2. Decisões fechadas (não reabrir sem motivo)

| Tópico | Decisão |
|--------|---------|
| Onde pode aparecer | **Somente** valor de parâmetro (`name: @JSON(...)`) |
| Payload | Texto **cru** da linguagem dentro de `(...)` — sem wrapping `"..."` obrigatório |
| Placeholder | **Somente** blob inteiro: `@JSON(#{blob})` |
| Interpolação parcial | **Proibida** — ex. `@JSON({"a": #{x}})` inválido |
| `use` / nomes de nó | Sem `@Lang`; `#{…}` de template continua como já existe |
| Object Rainbow `(a: 1)` | Continua com parênteses; **não** é JSON (`@JSON({"a":1})` é outra coisa) |
| Token `@` | `use Name@1.0.0` permanece; `@LANG(` é tagged value (peek Ident + `(`) |
| Allowlist v1 | `JSON`, `YAML`, `XML`, `HTML`, `MARKDOWN` (nomes longos; sem alias no v1) |

### Formas válidas de `use` (já implementadas — contexto)

- `use Name@1.0.0` (versão obrigatória no concreto)
- `use #{Name}` (template → expand → `use Name@SemVer`)
- `#{FullUseLine}` expandindo para `use Screen@1.0.0`
- Sem `use Name` sem versão; sem `use #{Name}@1.0.0`

### Expand (já implementado — contexto)

- CLI: `rainbow expand --map vars.json [--validate] file.rbw`
- API: `expand_placeholders` / `expand_and_validate` / `parse_substitution_map`
- Mesmo nome → mesma substituição em todas as ocorrências

---

## 3. Sintaxe canônica

```rainbow
use Screen@1.0.0

Screen(title: "Teste", variant: .name) {
  Text(text: "Teste")

  Button(
    title: "Teste",
    json: @JSON({"key": "value"}),
    jsonBlob: @JSON(#{jsonPlaceholder}),
    config: @YAML(enabled: true
list:
  - a),
    tree: @XML(<user id="1"><name>Ada</name></user>),
    body: @HTML(<div class="card">Hi</div>),
    note: @MARKDOWN(# Title

Hello **world**)
  ) {
    OnClick {
      SendHTTPRequest()
    }
  }
}
```

Grammar (alvo):

```text
Value       = … | TaggedValue
TaggedValue = "@" Language "(" Embedded ")"
Language    = "JSON" | "YAML" | "XML" | "HTML" | "MARKDOWN"
Embedded    = Placeholder | RawText   # RawText = linguagem cru, balanço de ( )
```

AST (alvo):

```text
RainbowValue::Tagged {
  language: EmbeddedLanguage,
  body: Text(String) | Placeholder(String),
  body_range: SourceRange,   # interior dos ( ) — obrigatório p/ erros + format
}
```

---

## 4. Cores das tags `@Lang` (locked)

Body: inject TextMate da linguagem. Tag `@JSON` / etc.: cor própria.

| Lang | Scope (tag) | Dark | Light |
|------|-------------|------|-------|
| JSON | `entity.name.type.embedded.json.rainbow` | `#FBBF24` | `#D97706` |
| YAML | `entity.name.type.embedded.yaml.rainbow` | `#34D399` | `#059669` |
| XML | `entity.name.type.embedded.xml.rainbow` | `#FB7185` | `#E11D48` |
| HTML | `entity.name.type.embedded.html.rainbow` | `#F97316` | `#EA580C` |
| MARKDOWN | `entity.name.type.embedded.markdown.rainbow` | `#38BDF8` | `#0284C7` |

Compartilhado:

- `@` / parens do tagged: cinza `#9CA3AF` / `#6B7280`
- `#{blob}`: placeholder existente `#67E8F9` / `#0E7490`

Preview: canvas `rainbow-embedded-lang-colors.canvas.tsx`.

Inject grammars:

- `@JSON(` → `source.json`
- `@YAML(` → `source.yaml`
- `@XML(` → `text.xml`
- `@HTML(` → `text.html.basic`
- `@MARKDOWN(` → `text.html.markdown`

---

## 5. Validação de erros — completa

Requisito do produto: **diagnóstico completo** de cada linguagem, com **range dentro dos `(...)`**.

### Estratégia (híbrida)

1. **rainbow-lsp (Rust)** — validators embutidos (sempre disponíveis no CLI/`validate --concrete`):
   - JSON → parse estrito (`serde_json`) + mensagens com offset mapeado
   - YAML → parse (`serde_yaml` / equivalente) + line/col → range
   - XML → well-formed (`roxmltree` / `quick-xml`)
   - HTML → fragment parse com relatório de erros (`html5ever` ou serviço dedicado)
   - Markdown → CommonMark strict o quanto possível (`comrak` / `pulldown-cmark` + regras extras)

2. **Extensão / LSPs de linguagem (opcional, “complete IDE”)** — preferência do usuário:
   - **Usar LSP do sistema / VS Code** quando disponível (JSON Language Features, YAML extension, etc.), **ou**
   - Empacotar language services na extensão (`vscode-json-languageservice`, yaml-language-server, …)
   - Mecânica: regiões embutidas → documento virtual (ou forward de `textDocument/diagnostic`) com `languageId` correto → diagnostics fundidos nos ranges Rainbow

Ordem prática na implementação:

1. Spans + validators Rust (CLI + LSP baseline).  
2. Virtual docs / language services na extensão para paridade “IDE completa”.  
3. Não bloquear ship se LSP externo ausente — Rust validators são a fonte da verdade do toolchain.

### Regras

- `@LANG(#{blob})` em template: **não** validar o interior até expand.  
- Pós-expand / `--concrete`: validação obrigatória.  
- Códigos: `rainbow.embedded.<lang>.<code>`  
- `source`: `rainbow-lsp` (e, se merge de LSP externo, preservar detalhe na mensagem)

---

## 6. Formatação

| Camada | Comportamento |
|--------|----------------|
| Envelope Rainbow | Printer atual (params ≥2 multilinha, braces, etc.) |
| Body válido | Pretty-print da linguagem (indent 2 onde fizer sentido) |
| Body inválido | **Não** reformatar o body; diagnosticar; formatar só o envelope |
| Placeholder | `@LANG(#{name})` estável |

Format Document / format on save da extensão continua no `rainbow-lsp` (um pipeline).

YAML/Markdown multilinha: regra de fechamento do `)` Rainbow documentada (`)` final do tagged; preferir linha própria em blocos longos se o scanner exigir).

---

## 7. Scanner do payload

Após `@LANG(`:

1. Se `#{Ident}` + `)` → body placeholder.  
2. Senão: raw text com:
   - balanço de `()`
   - respeito a strings/`'`/`"` / escapes da linguagem
   - XML/HTML: comentários, CDATA quando aplicável  
3. Gravar `body_range` (offsets absolutos no source).

---

## 8. Plano por linguagem (PRs)

### PR1 — Infra + JSON (MVP)

- Tagged AST + lexer/parser/printer + spans  
- Cores `@JSON` + inject `source.json`  
- Validate JSON completo (Rust) + diagnostics com range  
- Format pretty JSON  
- `@JSON(#{blob})` + expand  
- Testes / fixtures  
- Docs grammar + PARITY note  

### PR2 — YAML

- Cores + inject  
- Regra multilinha / `)`  
- Validate + format + diagnostics  

### PR3 — XML

- Cores + inject  
- Well-formed validate + pretty  
- Scanner strings/CDATA/comentários  

### PR4 — HTML

- Cores + inject  
- Fragment validate (erros reais do parser) + format conservador  

### PR5 — Markdown

- Cores + inject  
- Validate CM + format leve  
- Fechamento multilinha  

### PR6 — Extensão “IDE completa” ✅

- Bundled language services (JSON/YAML/HTML/Markdown/XML) in `editors/vscode-rainbow`  
- LSP `rainbow/embeddedRegions` + range mapping + diagnostic merge (filter Rust `rainbow.embedded.*` in editor)  
- Hover + completions inside `@LANG(...)`  
- README tabela lang → cor → format → validate / bundled IDE  
- Setting: `rainbow.embedded.bundledLanguageServices`

### PR7 — Parity Swift

- Espelhar tagged values no `mobile/iOS/packages/rainbowparser`  

---

## 9. Critérios de aceite (produto)

- [x] `@Lang` só em params; payload cru; só `#{blob}` inteiro  
- [x] Cada tag tem cor dark/light (canvas = referência visual)  
- [x] Body com syntax highlight da linguagem (TextMate inject)  
- [x] Format on save: envelope + body (se válido)  
- [x] Erro da lang → squiggle **dentro** dos `(...)` com mensagem específica  
- [x] Template hole sem false-positive de validate da lang  
- [x] CLI `expand` + `validate --concrete` cobrem o ship path  
- [x] Sem LSP externo, validators Rust ainda reportam erros  

PR6 (implementado): extensão empacota language services (`vscode-json-languageservice`,
`yaml-language-server`, `vscode-html-languageservice`, `htmlhint`, `markdownlint`,
`fast-xml-parser`). O LSP expõe `rainbow/embeddedRegions`; a extensão mapeia
diagnostics/hover/completion de volta ao `.rbw`. Com
`rainbow.embedded.bundledLanguageServices` (default), diagnostics `rainbow.embedded.*`
do Rust são filtrados no editor (CLI continua com validators Rust).

---

## 10. Fora de escopo (v1)

- Interpolação parcial dentro do payload  
- JSON Schema / XSD / HTMLHint completo como produto separado  
- `@Lang` em nomes de nó ou `use`  
- Aliases (`@MD`, `@Json`)  
- Heredoc `<<EOF` (só se YAML/MD multilinha exigir na v2)

---

## 11. Arquivos tocados (quando implementar)

- `crates/rainbow-parser/src/{ast,lexer,parser,printer,embedded,semantics,json,lib}.rs`  
- `crates/rainbow-lsp/src/backend.rs` (+ format embedded)  
- `crates/rainbow-cli` (validate concrete já existe)  
- `editors/vscode-rainbow/{syntaxes,themes,package.json,src}`  
- `spec/grammar.md`, `spec/ast.schema.json`, `docs/PARITY.md`  
- `fixtures/embedded/…`  
- Canvas (já criado): cores locked  

---

## 12. Relação com o que já existe

| Feature | Estado |
|---------|--------|
| `#{Name}` placeholder geral | Feito |
| `rainbow expand` | Feito |
| UpperCamelCase / plugin vs event | Feito |
| Formatter ≥2 params | Feito |
| `@Lang(...)` | **Não feito** — este plano |

---

## 13. Próximo passo quando for implementar

1. Abrir este arquivo + canvas de cores.  
2. Começar **PR1 (JSON + infra + spans + diagnostics + format + cores)**.  
3. Só então PR2–PR5.  
4. PR6 language services se a validação Rust não bastar na prática do editor.
