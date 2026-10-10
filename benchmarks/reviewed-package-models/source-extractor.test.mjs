import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { test } from "node:test";
import { SourceExtractor, callableValue, literal, objectValue } from "./source-extractor.mjs";

function specimen(text, name = "example", args = null, host = "browser", options = {}) {
  const dir = mkdtempSync(join(tmpdir(), "solid-source-extractor-")), path = join(dir, "index.js");
  writeFileSync(path, text);
  return new SourceExtractor(host, options).extract(path, name, args);
}

test("exact imported factory identity survives aliases and local helpers", () => {
  const result = specimen(`import { createSignal as signal } from "solid-js";
function helper() { const [read] = signal(0); return read; }
function example() { return helper(); } export { example };`);
  assert.equal(result.behavior.returns, "accessor");
  assert.equal(result.evidence[0].native, "solid-js.createSignal");
});

test("shadowing a native factory supplies no accessor premise", () => {
  const result = specimen(`import { createSignal } from "solid-js";
export function example() { function createSignal() { return [() => false]; } const [read] = createSignal(); return read; }`);
  assert.equal(result.behavior.returns, undefined);
});

test("host branches do not turn server fallback functions into accessors", () => {
  const source = `import { createSignal } from "solid-js"; import { isServer } from "@solidjs/web";
export function example() { if (isServer) return () => false; const [read] = createSignal(false); return read; }`;
  assert.equal(specimen(source).behavior.returns, "accessor");
  assert.equal(specimen(source, "example", null, "node").behavior.returns, undefined);
});

test("owner guards and root boundaries remove ambient ownership premises", () => {
  for (const body of [`getOwner() && onCleanup(() => {});`, `if (getOwner()) onCleanup(() => {});`, `createRoot(() => onCleanup(() => {}));`])
    assert.equal(specimen(`import { getOwner, onCleanup, createRoot } from "solid-js"; export function example() { ${body} }`).behavior.owner, undefined);
  assert.equal(specimen(`import { onCleanup } from "solid-js"; export function example() { onCleanup(() => {}); }`).behavior.owner, "cleanup");
});

test("options-dependent registration remains unknown until the argument is known", () => {
  const source = `import { onCleanup } from "solid-js"; export function example(options) { if (options.listen) onCleanup(() => {}); }`;
  assert.equal(specimen(source).behavior.owner, undefined);
  assert.equal(specimen(source, "example", [objectValue({ listen: literal(true) })]).behavior.owner, "cleanup");
  assert.equal(specimen(source, "example", [objectValue({ listen: literal(false) })]).behavior.owner, undefined);
});

test("deferred callback bodies are not interpreted in the factory's phase", () => {
  assert.equal(specimen(`import { onCleanup } from "solid-js"; export function example() { setTimeout(() => onCleanup(() => {}), 0); }`).behavior.owner, undefined);
});

test("mixed return origins and unsupported control flow remain explicit gaps", () => {
  const result = specimen(`import { createSignal } from "solid-js"; export function example(flag) { if (flag) return createSignal(0); return [() => false]; }`);
  assert.equal(result.behavior.returns, undefined);
  const loop = specimen(`import { createSignal, onCleanup } from "solid-js"; export function example(items) { for (const item of items) { if (item) return [() => false]; } onCleanup(() => {}); return createSignal(0); }`);
  assert.deepEqual(loop.behavior, {}); assert(loop.gaps.some(item => item.includes("ForOfStatement")));
});

test("recursive calls stop at a budget and supply no premise", () => {
  const result = specimen(`export function example() { return example(); }`, "example", null, "browser", { maxDepth: 3 });
  assert.deepEqual(result.behavior, {}); assert(result.gaps.some(item => item.includes("depth budget")));
});

test("conditional writes and unknown immediate callback timing do not preserve a stale return origin", () => {
  for (const action of [`flag ? read = () => false : 0;`, `const replace = () => { read = () => false; }; replace();`, `foreign(() => { read = () => false; });`]) {
    const result = specimen(`import { createSignal } from "solid-js"; export function example(flag) {
      const [source] = createSignal(0); let read = source; ${action} return read; }`);
    assert.equal(result.behavior.returns, undefined, action);
  }
});

test("relative re-exports follow runtime bodies and ambiguous stars remain unknown", () => {
  const dir = mkdtempSync(join(tmpdir(), "solid-source-exports-")), path = join(dir, "index.js");
  writeFileSync(join(dir, "helper.js"), `import { createSignal } from "solid-js"; export function example() { return createSignal(0); }`);
  writeFileSync(join(dir, "helper.d.ts"), `export declare function example(): [() => number];`);
  writeFileSync(path, `export { example } from "./helper.js";`);
  assert.equal(new SourceExtractor("browser").extract(path, "example").behavior.returns, "tuple0");
  writeFileSync(join(dir, "other.js"), `export function example() { return [() => false]; }`);
  writeFileSync(path, `export * from "./helper.js"; export * from "./other.js";`);
  assert.equal(new SourceExtractor("browser").extract(path, "example").behavior.returns, undefined);
});

test("spread options and tuple mutations cannot manufacture a return origin", () => {
  const spread = specimen(`import { createSignal } from "solid-js"; export function example(input) {
    const options = { reactive: true, ...input }; if (options.reactive) return createSignal(0); return [() => false]; }`);
  assert.equal(spread.behavior.returns, undefined);
  for (const change of [`tuple[0] = () => false;`, `read &&= () => false;`, `read++;`, `const options = { get x() { read = () => false; } }; options.x;`]) {
    const result = specimen(`import { createSignal } from "solid-js"; export function example() {
      const tuple = createSignal(0); let read = tuple[0]; ${change} return ${change.startsWith("tuple") ? "tuple" : "read"}; }`);
    assert.equal(result.behavior.returns, undefined);
  }
});

test("tuple premises retain every mandatory accessor position across return paths", () => {
  const source = `import { createSignal, createMemo } from "solid-js";
export function example(flag) { const [read, write] = createSignal(0); const memo = createMemo(() => 1);
  if (flag) return [read, memo, write]; return [read, memo, () => 0]; }`;
  assert.deepEqual(specimen(source).behavior, { returns: "tuple", accessorMembers: [0, 1] });
  assert.deepEqual(specimen(source.replace("[read, memo, () => 0]", "[read, () => 0, memo]")).behavior, { returns: "tuple0" });
  assert.deepEqual(specimen(`import { createMemo } from "solid-js"; export function example() { return [0, createMemo(() => 1)]; }`).behavior,
    { returns: "tuple", accessorMembers: [1] });
  assert.deepEqual(specimen(`import { createSignal, createMemo } from "solid-js"; export function example() {
    const pair = createSignal(0); return [...pair, createMemo(() => 1)]; }`).behavior, { returns: "tuple", accessorMembers: [0, 2] });
  assert.deepEqual(specimen(`import { createSignal, createMemo } from "solid-js"; export function example() {
    const pair = createSignal(0); return [...pair.slice(0, 1), createMemo(() => 1)]; }`).behavior, { returns: "tuple", accessorMembers: [0, 1] });
  const spread = specimen(`import { createMemo } from "solid-js"; export function example(items) { return [...items, createMemo(() => 1)]; }`);
  assert.deepEqual(spread.behavior, {}); assert(spread.gaps.includes("unmodeled array spread positions"));
});

test("typeof specializes concrete argument branches without declaring a generic owner requirement", () => {
  const source = `import { onCleanup, createEffect } from "solid-js"; export function example(delay) {
    if (typeof delay === "number") { onCleanup(() => {}); return; } createEffect(() => delay(), () => {}); }`;
  assert.deepEqual(specimen(source).behavior, {});
  assert.equal(specimen(source, "example", [literal(100)]).behavior.owner, "cleanup");
  assert.equal(specimen(source, "example", [callableValue()]).behavior.owner, "effect");
  const conditional = `import { onCleanup } from "solid-js"; export function example(option) { if (typeof option === "number") onCleanup(() => {}); }`;
  assert.equal(specimen(conditional, "example", [literal("100")]).behavior.owner, undefined);
});
