# Rainbow Grammar

Rainbow is a textual DSL for describing native SDUI payloads. This grammar is
syntax-only; semantic validation belongs to the application layer.

Parity target for **concrete** grammar: iOS `RainbowParser` and (future) Android.
Template holes (`#{Name}`) and `expand` are **backend-only** in this crate.

## Grammar

```text
Document      = UseDeclaration* Node*
UseDeclaration = "use" Identifier "@" Version
               | "use" Placeholder
Node          = NodeName Arguments? Children? Modifier*
NodeName      = Identifier | Placeholder
Arguments     = "(" ParameterList? ")"
ParameterList = Parameter ("," Parameter)*
Parameter     = Identifier ":" Value
Children      = "{" Node* "}"
Modifier      = "." "id" "(" String ")"
              | "." "style" Arguments
              | "." "events" "{" Trigger* "}"
              | "." "slots" "{" Slot* "}"
Trigger       = NodeName "{" Event* "}"
Event         = NodeName Arguments? IdModifier?
Slot          = NodeName "{" Node* "}"
IdModifier    = "." "id" "(" String ")"
Value         = String | Number | Bool | EnumIdentifier | Placeholder | TaggedValue | Array | Object | Null
TaggedValue   = "@" EmbeddedLanguage "(" EmbeddedBody ")"
EmbeddedLanguage = "JSON" | "YAML" | "XML" | "HTML" | "MARKDOWN"
EmbeddedBody  = Placeholder | RawEmbeddedText
Array         = "[" (Value ("," Value)* ","?)? "]"
Object        = "(" (ObjectEntry ("," ObjectEntry)*)? ")"
ObjectEntry   = ObjectKey ":" Value
ObjectKey     = Identifier | String
```

## Lexical Rules

```text
IdentifierStart        = "_" | "A"..."Z" | "a"..."z"
IdentifierContinuation = IdentifierStart | "0"..."9"
Identifier             = IdentifierStart IdentifierContinuation*
Placeholder            = "#{" Identifier "}"
EnumIdentifier         = "." Identifier
String                 = '"' character* '"'
Int                    = "-"? digit+
Double                 = "-"? digit+ "." digit+
Version                = digit+ "." digit+ "." digit+ ("." digit+)*
Bool                   = "true" | "false"
Null                   = "null"
At                     = "@"
TaggedValue            = "@" EmbeddedLanguage "(" EmbeddedBody ")"
Whitespace             = " " | "\n" | "\r\n" | "\r" | "\t"
```

Notes:

- `1.25` lexes as `Double`; `1.0.0` lexes as `Version` (two or more dots).
- Enum / bare identifier **values** require a leading `.` (e.g. `.primary`). A bare
  `primary` is not a value. The AST stores the name without the dot (`"primary"`).
- Top-level `use` starts a pin declaration; inside blocks, `use` remains a normal identifier/node name.
- `use` is optional. Semantic layers resolve unpinned names to the latest registered version.
- Line comments use `//` through end of line (like many C-family languages). `#` alone is
  not a comment — `#` starts a `#{Name}` placeholder.
- Templates may use `#{Name}` holes. **This is backend-only.** The Rust crate
  (global API) expands holes before shipping to mobile. iOS and Android parsers
  must accept **concrete** Rainbow only — they must not implement `expand` or
  treat `#{Name}` as a mobile runtime substitution API:
  - `use Name@1.0.0` — concrete SemVer pin (**required** for concrete names)
  - `use #{Name}` — template pin; expand map value must be `Name@MAJOR.MINOR.PATCH`
    (e.g. `PromoBanner@1.0.0`) → becomes `use PromoBanner@1.0.0`
  - Unversioned `use Name` is a **parse error**
  - `#{Name}` as a whole statement/node/value — same key replaces every occurrence
  - Do **not** mix `use #{Name}@1.0.0`
  - Full-line pins: `#{UsePin}` → `use Screen@1.0.0` when the map value is that whole line
- Plugin, event, trigger, and concrete `use` pin names must be **UpperCamelCase** ASCII
  letters only (e.g. `Screen`, `OnTap`, `SendHTTPRequest`). Parameter names,
  enum values (`.name`), and placeholder keys are not constrained by this rule.
- Every concrete **plugin** and **event** call without a children block is a
  constructor. The argument list is required even when it is empty: `Spinner()`
  and `Navigate()`, never `Spinner` or `Navigate`. A missing `()` is
  `rainbow.semantic.missingConstructor`.
  A plugin that opens a children block may omit the empty argument list.
  `Card { Text() }` and `Card() { Text() }` are both valid. The `{` stays on
  the same line as the name, or on the same line as `)` when the call has
  parentheses. Events do not have children blocks, so they still require `()`.
  `On*` triggers are not constructors (`OnTap { Navigate() }`).
- A trigger exists to hold actions. It must contain at least one child.
  `OnTap { }` and a bare `OnTap` are `rainbow.semantic.emptyTrigger`.
  A trigger with one or more events is valid. Empty `{}` blocks on plugins
  stay legal (`Empty() { }`).
- `#{Name}` holes skip the constructor check until expand. When the map value
  is only a plugin or event name, write the call in the template:
  `#{Plugin}()` → `PromoBanner()`. A substitution that is already a full call
  replaces the hole by itself and must include its own `()`.
- Concrete validation (`validate --concrete` / `expand --validate`) rejects leftover `#{…}`.
- Embedded `@LANG(...)` values appear **only** as parameter values (not node/`use` names):
  - Payload is raw language text inside `(...)` (not a Rainbow string).
  - Whole-blob template: `@JSON(#{Name})` — expand inserts the map value inside `@JSON(...)`.
  - Partial interpolation inside the payload (e.g. `@JSON({"a": #{x}})`) is a lexer error.
  - Allowlist: `JSON`, `YAML`, `XML`, `HTML`, `MARKDOWN` (no aliases in v1).
  - Rainbow object `(a: 1)` is a native Rainbow value (parentheses, not braces). JSON
    payloads must use `@JSON({"a":1})`. Do not treat bare `{…}` as a Rainbow object or as JSON.
  - A trailing comma after the last object entry is a parse error (same rule as parameters).
  - `use Name@1.0.0` still uses a bare `@` token; `@LANG(` is recognized by peeking Ident + `(`.
  - Validators live in the Rust toolchain (`validate` / LSP); invalid bodies keep original text on format.
- Plugin and event names share one UpperCamelCase namespace. Kinds are positional
  (not in the AST): non-`On*` roots/body children are **plugins**; non-`On*`
  children of `On*` triggers are **events**; `On*` nodes are triggers.
  Tooling (`validate` / LSP) errors if the same name appears in both positions,
  or if a node/`use` name is not UpperCamelCase.
  A name registry JSON (`plugins` / `events`) covers backends without a document
  tree; iOS `NShiftRegisteredVersionCatalog` rejects dual registration at runtime.

Supported string escapes:

```text
\" \\ \n \r \t \0
```

## Canonical Formatting

- `use` declarations are printed before nodes, one per line: `use Screen@1.0.0`.
- Consecutive `use` lines stay packed (no blank line between them). A blank line
  separates the last `use` from the first root node.
- Sibling nodes (roots and block children) are separated by one blank line.
- Leading `//` line comments on `use` / nodes / parameters are preserved and
  printed immediately above the construct at the same indent.
- Nodes are indented with two spaces per level.
- A single scalar parameter stays inline: `Button(title: "Entrar")` and
  `Progress(value: 0.66)`, unless its value breaks (multiline `@LANG(...)`, or
  an object that breaks).
- A plugin or event with no parameters is still a constructor: `Spinner()` and
  `Separator()`. Triggers with no parameters stay `OnTap` (no empty `()`).
- When a plugin/event has **two or more** parameters, **or** any parameter
  value breaks, each parameter is on its own indented line (**no** trailing
  comma after the last parameter). A call with one parameter breaks when that
  parameter's value breaks:
  ```text
  Screen(
    title: "Exemplo",
    variant: .primary
  ) {
    …
  }

  VerticalStack(
    style: (
      alignment: .leading,
      spacing: 12
    )
  )
  ```
- An object with **two or more** fields always breaks. Each field is on its
  own line. The closing `)` aligns with the indent of the line that opened
  the object (the parameter, or the parent field). Nested objects break at
  every level that has two or more fields, and a one-field object also breaks
  when its value breaks:
  ```text
  navigationBar: (
    title: "Feedback",
    theme: "amber",
    leading: "back"
  )
  ```
- An object with zero or one field stays compact when that field's value is
  single-line: `(id: "home")` and `()`.
- A trailing comma after the last parameter is a parse error.
- A trailing comma after the last object entry is a parse error.
- `On*` triggers stay compact when every parameter value is single-line.
  A breaking value still breaks the trigger call.
- Blocks always expand braces with newlines (including empty plugin blocks):
  `Empty() {\n}` — never `Empty() {}` on one line. An empty trigger is illegal;
  the printer still expands the braces if one is encoded.
- Arrays stay inline (`[…]`). Objects use `(…)`.
- Object keys are printed as identifiers when possible, otherwise as strings.
- Multiline `@LANG(...)` bodies pretty-print with continuation lines indented to
  the parameter indent; the lexer dedents those lines on re-parse.
