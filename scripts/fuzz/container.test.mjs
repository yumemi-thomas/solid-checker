import { describe, expect, it } from "vitest";
import { applyContainers, containerEvidence, wrapAt } from "./container.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { CONTAINER_KINDS, validate, walk } from "./model.mjs";
import { printProgram } from "./print.mjs";
import { createRng } from "./rng.mjs";

const seeds = Array.from({ length: 400 }, (_, i) => i + 1);
const containers = (seed) => generateProgram(seed, { profile: "containers" });
const nodes = (program) => [...walk(program)].map(({ node }) => node);

describe("the containers profile", () => {
  it("is deterministic and only produces valid, neutral programs", () => {
    for (const seed of seeds) {
      const program = containers(seed);
      expect(validate(program), `seed ${seed}`).toEqual([]);
      const printed = printProgram(program);
      expect(printed.text).toBe(printProgram(containers(seed)).text);
      expect(stripInstrumentation(printed.runText), `seed ${seed}`).toBe(printed.text);
    }
  });

  it("draws every container kind, call form, binding and base profile", () => {
    const all = seeds.flatMap((seed) => nodes(containers(seed)));
    expect(new Set(all.filter((n) => n.k === "contain").map((n) => n.kind))).toEqual(new Set(CONTAINER_KINDS));
    expect(new Set(all.filter((n) => n.k === "call" && n.via).map((n) => n.via))).toEqual(new Set(["optional", "as", "comma", "and", "paren"]));
    expect(all.some((n) => n.k === "decl" && n.kw === "let")).toBe(true);
    expect(all.some((n) => n.k === "decl" && n.e.k === "fn" && n.e.form === "function")).toBe(true);
    expect(all.some((n) => n.k === "contain" && n.async)).toBe(true);
    const used = new Set(seeds.map((seed) => containers(seed).components[0].shape?.where));
    expect(used.has("memo") && used.has("show")).toBe(true);
  });

  it("passes callbacks by name to the primitives the generator uses", () => {
    const text = Array.from({ length: 1500 }, (_, i) => printProgram(containers(i + 1)).text).join("\n");
    const forms = {
      memo: /createMemo\(cb\d+\)/, effectCompute: /createEffect\(cb\d+, /, effectApply: /createEffect\(.*, cb\d+\)/, root: /createRoot\(cb\d+\)/,
      owner: /runWithOwner\((owner|null|createOwner\(\)), cb\d+\)/, untrack: /untrack\(cb\d+\)/, cleanup: /onCleanup\(cb\d+\)/, settled: /onSettled\(cb\d+\)/,
      timer: /setTimeout\(cb\d+, 0\)/, then: /\.then\(cb\d+\)/, micro: /queueMicrotask\(cb\d+\)/, forEach: /\.forEach\(cb\d+\)/, helper: /\bh\d+\(cb\d+\)/,
      let: /^\s+let cb\d+ = /m, declaration: /^\s+function cb\d+\(\) /m,
    };
    for (const [name, pattern] of Object.entries(forms)) expect(pattern.test(text), name).toBe(true);
  });

  it("leaves every program alone when containers are off", () => {
    for (const profile of ["disposal", "owner-cleanup", "mixed", "closure-escape"]) {
      for (const seed of seeds.slice(0, 60)) {
        const found = nodes(generateProgram(seed, { profile }));
        expect(found.some((n) => n.k === "contain" || (n.k === "call" && n.via) || n.kw), `${profile} ${seed}`).toBe(false);
      }
    }
  });

  it("opts in on top of another profile", () => {
    const program = generateProgram(5, { profile: "disposal", containers: true });
    expect(validate(program)).toEqual([]);
    expect(program.components[0].shape).toBeDefined();
    const withContainers = seeds.filter((seed) => nodes(generateProgram(seed, { profile: "owner-cleanup", containers: true })).some((n) => n.k === "contain"));
    expect(withContainers.length).toBeGreaterThan(100);
  });
});

/** The disposal component of a handle called at once in a memo: `const h1 = onCleanup(...); h1();`. */
const handleCalledInMemo = () => generateProgram(100, {
  profile: "disposal",
  disposalPlan: { via: "signal", where: "memo", cleanupVia: "onCleanup", handleMode: "call-now", wrappers: [], usesTrigger: true },
});
const handleCall = (program) => {
  for (const { node } of walk(program)) {
    if (node.k === "fn" && node.body.some((s) => s.k === "expr" && s.e.k === "call" && s.e.callee.k === "ref" && s.e.callee.name === "h1")) {
      return { list: node.body, index: node.body.findIndex((s) => s.k === "expr" && s.e.k === "call") };
    }
  }
  throw new Error("no handle call");
};

describe("one container around a cleanup handle's call", () => {
  const shapes = {
    "comp-never": [/^ {4}function Child1\(\) \{\n {6}h1\(\);\n {6}return <span \/>;\n {4}\}$/m],
    "comp-rendered": [/function Child1\(\) \{/, /^ {4}<Child1 \/>;$/m],
    "comp-slot": [/function Child1\(\) \{/, /^ {4}\(\{ slot: \(c: \(\) => unknown\) => c\(\) \}\)\.slot\(Child1\);$/m],
    "comp-prop": [/export function C1\(props: \{ slot\?: /, /^ {4}props\.slot\?\.\(Child1\);$/m],
    "comp-returned": [/\(\(\) => \{\n {6}function Child1\(\) \{/, /return Child1;\n {4}\}\)\(\);/],
    "class-field": [/class K1 \{\n {6}f = h1\(\);\n {4}\}\n {4}new K1\(\);/],
    "class-static": [/class K1 \{\n {6}static f = h1\(\);\n {4}\}/],
    "class-sblock": [/class K1 \{\n {6}static \{\n {8}h1\(\);\n {6}\}\n {4}\}/],
    "class-method": [/class K1 \{\n {6}m\(\) \{\n {8}h1\(\);\n {6}\}\n {4}\}\n {4}new K1\(\)\.m\(\);/],
    "class-ctor": [/class K1 \{\n {6}constructor\(\) \{\n {8}h1\(\);\n {6}\}\n {4}\}\n {4}new K1\(\);/],
    "dead-if": [/^ {4}if \(false\) \{\n {6}h1\(\);\n {4}\}$/m],
    "dead-after-return": [/\(\(\) => \{\n {6}return;\n {6}h1\(\);\n {4}\}\)\(\);/],
    "dead-fn": [/^ {4}function dead1\(\) \{\n {6}h1\(\);\n {4}\}$/m],
    "and-true": [/^ {4}true && h1\(\);$/m],
    "and-false": [/^ {4}false && h1\(\);$/m],
  };
  const uses = { "comp-returned": "discard", "class-field": "new", "class-ctor": "new", "class-method": "call" };

  for (const kind of CONTAINER_KINDS) {
    it(`${kind} prints its construct, validates and strips back to the clean text`, () => {
      const program = handleCalledInMemo();
      const { list, index } = handleCall(program);
      wrapAt(list, index, kind, 1, { use: uses[kind] ?? null });
      expect(validate(program), kind).toEqual([]);
      const printed = printProgram(program);
      for (const pattern of shapes[kind]) expect(printed.text, kind).toMatch(pattern);
      expect(stripInstrumentation(printed.runText)).toBe(printed.text);
    });
  }

  it("puts an await before or after the payload of an async container", () => {
    for (const [gap, pattern] of [["before", /async function Child1\(\) \{\n {6}await Promise\.resolve\(\);\n {6}h1\(\);/], ["after", /h1\(\);\n {6}await Promise\.resolve\(\);\n {6}return <span \/>;/]]) {
      const program = handleCalledInMemo();
      const { list, index } = handleCall(program);
      wrapAt(list, index, "comp-never", 1, { async: true, gap });
      expect(validate(program)).toEqual([]);
      expect(printProgram(program).text).toMatch(pattern);
    }
    const program = handleCalledInMemo();
    const { list, index } = handleCall(program);
    wrapAt(list, index, "comp-rendered", 1, { async: true });
    expect(validate(program)).not.toEqual([]);
  });

  it("counts a function-like container's entry, so a container that never ran is told from one that did", () => {
    const program = handleCalledInMemo();
    const { list, index } = handleCall(program);
    wrapAt(list, index, "comp-never", 1);
    const built = buildCase(program, { id: "k1" });
    const hits = Object.fromEntries(built.ledger.map((row) => [row.hit, 0]));
    expect(containerEvidence(program, built.ledger, hits)).toEqual([{ kind: "comp-never", entered: 0 }]);
    const entry = built.ledger.find((row) => row.kind === "entry" && row.node === "contain");
    hits[entry.hit] = 1;
    expect(containerEvidence(program, built.ledger, hits)[0].entered).toBe(1);
    expect(containerEvidence(program, built.ledger, null)[0].entered).toBeNull();
  });

  it("a direct field initializer has no entry counter, its call's glue counter says it ran", () => {
    const program = handleCalledInMemo();
    const { list, index } = handleCall(program);
    wrapAt(list, index, "class-field", 1, { use: "new" });
    const built = buildCase(program, { id: "k2" });
    expect(built.runText).toMatch(/f = \(__h\(\d+\), h1\(\)\);/);
    expect(containerEvidence(program, built.ledger, {})[0].entered).toBeNull();
  });

  it("rewrites calls and moves callbacks without a container in the way", () => {
    const rng = createRng(3);
    const program = handleCalledInMemo();
    const ctx = { counters: {} };
    applyContainers(ctx, program.components[0], { rng, count: [[1, 0]] });
    expect(validate(program)).toEqual([]);
  });
});
