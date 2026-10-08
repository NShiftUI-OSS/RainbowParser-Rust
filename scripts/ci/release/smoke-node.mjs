import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const require = createRequire(import.meta.url);
const entry = process.argv[2];
if (!entry) {
  console.error("usage: node smoke-node.mjs <rainbow_parser_wasm.js>");
  process.exit(1);
}

const rainbow = require(entry);

function parseJson(label, raw) {
  let value;
  try {
    value = JSON.parse(raw);
  } catch (error) {
    console.error(`${label}: response is not JSON`, raw);
    throw error;
  }
  return value;
}

function assertOk(label, raw) {
  const value = parseJson(label, raw);
  if (!value.ok) {
    console.error(`${label}: expected ok`, raw);
    process.exit(1);
  }
  return value;
}

const version = rainbow.version();
if (typeof version !== "string" || version.length === 0) {
  console.error("version() did not return a SemVer string", version);
  process.exit(1);
}

const parsed = assertOk("parse", rainbow.parse('Button(title: "Entrar")'));
if (!JSON.stringify(parsed).includes("Button")) {
  console.error("parse JSON missing Button", parsed);
  process.exit(1);
}

const broken = parseJson("parse error", rainbow.parse("Button("));
if (broken.ok !== false) {
  console.error("parse should report ok:false for a syntax error", broken);
  process.exit(1);
}

assertOk("format", rainbow.format('Button(title:"Entrar")'));
assertOk("validate", rainbow.validate("Button(title: #{Title})", false));

const concrete = parseJson(
  "validate concrete",
  rainbow.validate("Button(title: #{Title})", true),
);
if (concrete.ok !== false) {
  console.error("concrete validate should reject a placeholder", concrete);
  process.exit(1);
}

const expanded = assertOk(
  "expand",
  rainbow.expand("Button(title: #{Title})", '{"Title":"\\"Entrar\\""}', true),
);
if (!expanded.expanded.includes("Entrar")) {
  console.error("expand did not insert the fragment", expanded);
  process.exit(1);
}

assertOk(
  "validateRegistry",
  rainbow.validateRegistry(
    '{"plugins":["Screen","Button"],"events":["Navigate"]}',
  ),
);

console.log(`node wasm smoke ok (${version}) ${pathToFileURL(entry).href}`);
