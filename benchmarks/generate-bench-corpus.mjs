#!/usr/bin/env bun
// Deterministic synthetic Solid corpus for solid-checker-session-bench.
//
// Generates a chain-import project: mod<i> imports the helper from
// mod<i-1>. Per-file patterns exercise cleanup returns, named cleanup
// helpers (the symbol->function lookup), effects, and JSX reads.
//
// Keep per-file content slim: a heavier variant of this corpus produced a
// TypeFacts response over the 64 MiB frame limit at 5,000 files.
//
// Usage:
//   bun benchmarks/generate-bench-corpus.mjs 5000 /tmp/bench-corpus-5k [widget|structured]
//   rust/target/release/solid-checker-session-bench \
//     --project /tmp/bench-corpus-5k/tsconfig.json \
//     --typefacts bin/solid-typefacts \
//     --iterations 15 --warmups 3 \
//     [--edit /tmp/bench-corpus-5k/mod2500.tsx --edit-mode same-span-body]
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const count = Number(process.argv[2] ?? 5000);
const out = process.argv[3];
// "widget" is the default corpus every gate but export scaling uses.
// "structured" gives every module exported factories that return objects and
// tuples (shorthand properties), the shape export
// aggregation works on (plus a wrapper, a projection and a nested result per
// module); the widget corpus has none, so it exercises only the scan of
// exported functions. Export scaling (performance-invariants.mjs)
// counts that phase's work on this shape.
const shape = process.argv[4] ?? "widget";
if (!["widget", "structured"].includes(shape)) {
  console.error(`shape must be widget or structured, got ${shape}`);
  process.exit(2);
}
if (!out || !Number.isInteger(count) || count < 1) {
  console.error("usage: generate-bench-corpus.mjs <count> <outdir>");
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const shim = readFileSync(
  join(root, "fixtures/engine/eslint-reactivity-v2/solid-js.d.ts"),
);
writeFileSync(join(out, "solid-js.d.ts"), shim);

writeFileSync(
  join(out, "tsconfig.json"),
  JSON.stringify(
    {
      compilerOptions: {
        strict: true,
        jsx: "preserve",
        moduleResolution: "bundler",
        module: "esnext",
        target: "es2022",
        noEmit: true,
      },
      include: ["**/*.ts", "**/*.tsx"],
    },
    null,
    2,
  ) + "\n",
);

const pad = (i) => String(i).padStart(4, "0");

function structuredModule(i) {
  const n = pad(i);
  return `import { createSignal } from "solid-js";

export const cfg${n} = { index: ${i} };

export function make${n}() {
  const [count, setCount] = createSignal(${i});
  return { count, setCount, cfg: cfg${n} };
}

// Propagation through the factory above: a wrapper, a projection, and nested
// results, so summary instantiation and the discovery rounds have work.
export function wrap${n}() {
  return make${n}();
}

export function wrapTwice${n}() {
  return wrap${n}();
}

export function wrapThrice${n}() {
  return wrapTwice${n}();
}

export function project${n}() {
  const made = make${n}();
  return { count: made.count };
}

export function projectWrapped${n}() {
  const made = wrapTwice${n}();
  return { count: made.count, setCount: made.setCount };
}

export function nested${n}() {
  const inner = wrap${n}();
  return { outer: { inner, again: project${n}() } };
}

export function nestedDeep${n}() {
  return { level: { inner: { wrapped: wrapThrice${n}(), projected: projectWrapped${n}() } } };
}

export function pair${n}() {
  const [value, setValue] = createSignal(${i % 7});
  return [value, setValue];
}
`;
}

for (let i = 0; i < count; i++) {
  const name = `mod${pad(i)}`;
  const prev = i > 0 ? `mod${pad(i - 1)}` : null;
  const importPrev = prev
    ? `import { helper${pad(i - 1)} } from "./${prev}";\n`
    : "";
  const usePrev = prev ? `const upstream = helper${pad(i - 1)}();\n  ` : "";
  const source = shape === "structured" ? structuredModule(i) : `import { createSignal, createEffect } from "solid-js";
${importPrev}
export function helper${pad(i)}() {
  const [count] = createSignal(${i});
  return () => count() + ${i % 7};
}

function makeCleanup${pad(i)}() {
  return () => {};
}

export function Widget${pad(i)}() {
  ${usePrev}const [value, setValue] = createSignal(0);
  createEffect(
    () => value(),
    () => makeCleanup${pad(i)}(),
  );
  return (
    <div>
      <button onClick={() => setValue(value() + 1)}>{value()}</button>
      ${prev ? `<span>{upstream()}</span>` : ``}
    </div>
  );
}
`;
  writeFileSync(join(out, `${name}.tsx`), source);
}

writeFileSync(
  join(out, "jsx.d.ts"),
  `declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
    span: {};
  }
  type Element = unknown;
}
`,
);

console.log(`generated ${count} files in ${out}`);
