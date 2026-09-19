import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { offsetAt, positionAt, positionToOffset, offsetToPosition } = require(
  "../out/embedded/offsets.js"
);

assert.equal(offsetAt("ab\ncd", { line: 0, character: 2 }), 2);
assert.equal(offsetAt("ab\ncd", { line: 1, character: 1 }), 4);
assert.deepEqual(positionAt("ab\ncd", 4), { line: 1, character: 1 });

const source = 'x: @JSON({"a":1})\n';
const body = '{"a":1}';
const bodyStart = source.indexOf(body);
assert.equal(bodyStart > 0, true);
const local = positionAt(body, 1); // '{'
const abs = offsetToPosition(source, bodyStart + 1);
assert.deepEqual(abs, { line: 0, character: bodyStart + 1 });
assert.equal(positionToOffset(source, abs), bodyStart + 1);
assert.deepEqual(local, { line: 0, character: 1 });

console.log("offsets tests ok");
