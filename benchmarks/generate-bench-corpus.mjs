#!/usr/bin/env node
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
//   node benchmarks/generate-bench-corpus.mjs 5000 /tmp/bench-corpus-5k
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

for (let i = 0; i < count; i++) {
  const name = `mod${pad(i)}`;
  const prev = i > 0 ? `mod${pad(i - 1)}` : null;
  const importPrev = prev
    ? `import { helper${pad(i - 1)} } from "./${prev}";\n`
    : "";
  const usePrev = prev ? `const upstream = helper${pad(i - 1)}();\n  ` : "";
  const source = `import { createSignal, createEffect } from "solid-js";
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
