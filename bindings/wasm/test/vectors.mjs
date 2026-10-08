// Runs every reference vector through the WebAssembly build, from Node.
// Build first:
//   cargo build --release -p setoff-engine-wasm --target wasm32-unknown-unknown
//   wasm-bindgen --target nodejs --out-dir bindings/wasm/pkg-node \
//     target/wasm32-unknown-unknown/release/setoff_engine_wasm.wasm
// then: node --test bindings/wasm/test/
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";

const require = createRequire(import.meta.url);
const { net } = require("../pkg-node/setoff_engine_wasm.js");
const dir = new URL("../../../tests/vectors/", import.meta.url);
const vectors = readdirSync(dir).filter((f) => f.endsWith(".json"));

test("there are reference vectors to run", () => assert.ok(vectors.length >= 5));

for (const file of vectors) {
  const v = JSON.parse(readFileSync(new URL(file, dir), "utf8"));
  test(`${v.name} nets identically in WebAssembly`, () => {
    const out = JSON.parse(net(JSON.stringify(v.obligations)));
    assert.equal(out.ok, true);
    assert.deepEqual(out.netting, v.netting);
  });
}

test("a bad row is reported against its index", () => {
  const out = JSON.parse(net(JSON.stringify([
    { id: "1", debtor: "A", creditor: "B", asset: "USDC", amount: "100" },
    { id: "2", debtor: "B", creditor: "C", asset: "USDC", amount: "1.5" },
  ])));
  assert.equal(out.ok, false);
  assert.equal(out.errors[0].index, 1);
});
