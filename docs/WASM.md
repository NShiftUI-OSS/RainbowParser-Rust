# Rainbow WASM (Node and React)

`crates/rainbow-wasm` compiles the parser to WebAssembly. The functions return the same JSON envelopes as `rainbow <command> --json`. A syntax or semantic failure is `"ok": false` in that JSON. The call does not throw.

| Export | CLI equivalent |
| --- | --- |
| `version()` | `rainbow --version` |
| `parse(source)` | `rainbow parse --json` |
| `format(source)` | `rainbow format --json` |
| `validate(source, concrete)` | `rainbow validate --json` (`concrete` = `--concrete`) |
| `expand(source, mapJson, validate)` | `rainbow expand --map … --json` (`validate` = `--validate`) |
| `validateRegistry(source)` | `rainbow validate-registry --json` |

`expand` is the backend substitution step (`#{Name}` → concrete Rainbow). iOS and Android never see the placeholders. Do that substitution in Node before shipping a payload to mobile. The React app can `parse`, `format`, and `validate` locally so the editor stays responsive.

## Release artifacts

GitHub Actions **Release** is one pipeline. It builds both packages in the same job and publishes them on one GitHub Release. If either build fails, that release is not created. **Delete release** removes the GitHub Release, which deletes both packages together.

- `rainbow-parser-wasm-node.tgz` — Node.js CommonJS, for the backend
- `rainbow-parser-wasm-web.tgz` — ES module with `init()`, for Vite / React

```bash
npm install https://github.com/NShiftUI-OSS/RainbowParser-Rust/releases/download/v<tag>/rainbow-parser-wasm-node.tgz
npm install https://github.com/NShiftUI-OSS/RainbowParser-Rust/releases/download/v<tag>/rainbow-parser-wasm-web.tgz
```

Replace `<tag>` with the SemVer you published (`v0.1.0-beta.2`). The asset names stay stable across versions.

## Node backend

The node package loads the `.wasm` file itself. No `init()` call.

```js
const rainbow = require("rainbow-parser-wasm-node");

const parsed = JSON.parse(rainbow.parse('Button(title: "Entrar")'));
if (!parsed.ok) {
  // parsed.diagnostics
}

const expanded = JSON.parse(
  rainbow.expand(
    "Button(title: #{Title})",
    JSON.stringify({ Title: '"Entrar"' }),
    true,
  ),
);
// expanded.expanded is concrete Rainbow for iOS / Android
```

From an ES module:

```js
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const rainbow = require("rainbow-parser-wasm-node");
```

Map values are Rainbow fragments inserted verbatim. A string value includes the quotes (`"Entrar"`). A node fragment is source text (`PromoBanner`).

## React (Vite)

Install `rainbow-parser-wasm-web`. Call `init()` once. After that, `parse` and `validate` are synchronous. Vite emits the `.wasm` from `new URL(..., import.meta.url)` with the default config.

```jsx
import { useEffect, useState } from "react";
import init, { parse, validate } from "rainbow-parser-wasm-web";

let ready = null;
function loadParser() {
  ready ??= init();
  return ready;
}

export function RainbowPreview({ source }) {
  const [state, setState] = useState(null);

  useEffect(() => {
    let cancelled = false;
    loadParser().then(() => {
      if (cancelled) return;
      setState({
        parsed: JSON.parse(parse(source)),
        validated: JSON.parse(validate(source, false)),
      });
    });
    return () => {
      cancelled = true;
    };
  }, [source]);

  if (!state) return null;
  return <pre>{JSON.stringify(state, null, 2)}</pre>;
}
```

A copy of the component lives in `examples/react/RainbowPreview.jsx`. Leave placeholder expansion on the Node backend; the editor only needs parse, format, and validate.

## Local build

The Release job runs `scripts/ci/release/assemble.sh`. That needs Rust 1.88 or newer, the `wasm32-unknown-unknown` target, `wasm-pack` 0.15, and Node 20.

```bash
VERSION=0.1.0-beta.1 bash scripts/ci/release/assemble.sh
```

`cargo test --workspace` covers the JSON API on the host. From `crates/rainbow-wasm`, `wasm-pack test --node` runs the parser inside Wasm under Node.
