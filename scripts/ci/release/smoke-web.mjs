import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node smoke-web.mjs <web-package-dir>");
  process.exit(1);
}

const mod = await import(pathToFileURL(`${dir}/rainbow_parser_wasm.js`).href);
const bytes = readFileSync(`${dir}/rainbow_parser_wasm_bg.wasm`);
mod.initSync({ module: bytes });

const version = mod.version();
if (typeof version !== "string" || version.length === 0) {
  console.error("version() did not return a SemVer string", version);
  process.exit(1);
}

const parsed = JSON.parse(mod.parse('Button(title: "Entrar")'));
if (!parsed.ok) {
  console.error("parse failed", parsed);
  process.exit(1);
}

const concrete = JSON.parse(mod.validate("Button(title: #{Title})", true));
if (concrete.ok !== false) {
  console.error("concrete validate should reject a placeholder", concrete);
  process.exit(1);
}

console.log(`web wasm smoke ok (${version})`);
