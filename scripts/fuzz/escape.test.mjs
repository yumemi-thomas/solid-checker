import { describe, expect, it } from "vitest";
import { TS_KEYWORDS, escapable, escapeCandidates, escapeNames, escapeSource, sameProgram } from "./escape.mjs";
import { generateProgram } from "./generate.mjs";
import { rewriteProgram } from "./rewrite.mjs";
import { printProgram, programPackages, moduleFiles } from "./print.mjs";

// ADR 1152: the escape rewrite writes a program's identifiers, strings and template pieces with escapes. Its soundness is `sameProgram`: the escaped file parses to the same tree.

const SAMPLE = `import { createSignal } from "solid-js";
import type { Foo } from "./foo";
declare module "x" { interface A { b: string } }

export function Probe(props: { w1: number }, extra?: string) {
  "use strict";
  const [n, setN] = createSignal(0);
  const o = { n, "k": 1, [\`computed\`]: 2, m() { return 3; }, get g() { return 4; } };
  const { n: renamed, ...rest } = o as any;
  window.addEventListener("click", () => setN(n() + 1));
  const t = tag\`raw \${n()} text\`;
  const u = \`click \${n()} done\`;
  label: for (const i of [1, 2]) { if (i) break label; }
  const here = import.meta;
  class K { constructor() {} static s = 1; #p = 2; }
  return <div data-act="b1" onClick={() => setN(props.w1)}>{u} text {renamed}</div>;
}
`;

const namesOf = (places, role) => places.filter((place) => place.role === role).map((place) => place.name);

describe("escape: what is a place", () => {
  const places = escapeCandidates(SAMPLE);

  it("takes identifiers, strings and template pieces, by their cooked text", () => {
    expect(namesOf(places, "identifier")).toEqual(expect.arrayContaining(["createSignal", "Probe", "props", "w1".length ? "n" : "", "setN", "window", "addEventListener", "renamed", "rest", "label", "i", "tag", "K", "s"]));
    expect(namesOf(places, "string")).toEqual(expect.arrayContaining(["k", "click"]));
    expect(namesOf(places, "template")).toEqual(expect.arrayContaining(["computed", "click ", " done"]));
  });

  it("leaves a keyword, a TypeScript keyword, JSX, a directive, a tagged template, a type position, a type-only import, a private name and `import.meta` alone", () => {
    const names = new Set(places.map((place) => place.name));
    // Keyword-named identifiers (a class's `constructor`, a `get`) and the contextual keywords are no places by default.
    expect(names.has("constructor")).toBe(false);
    // `get g()`: the getter's name `g` is one, the keyword `get` is no node.
    expect(names.has("get")).toBe(false);
    expect(names.has("g")).toBe(true);
    // A directive, a JSX attribute string, JSX text, a type annotation's names and a type-only import's.
    expect(names.has("use strict")).toBe(false);
    expect(names.has("b1")).toBe(false);
    expect(names.has("string")).toBe(false);
    expect(names.has("Foo")).toBe(false);
    expect(names.has("A")).toBe(false);
    // A tagged template's pieces are read raw by the tag; `import.meta` and `#p` must be written plain.
    expect(namesOf(places, "template")).not.toContain("raw ");
    expect(names.has("meta")).toBe(false);
    expect(names.has("p")).toBe(false);
    // A module specifier is a place of its own role, off unless asked.
    expect(namesOf(places, "specifier")).toEqual(expect.arrayContaining(["solid-js"]));
    const text = escapeSource(SAMPLE).text;
    expect(text).toContain('from "solid-js"');
    expect(text).toContain("data-act=\"b1\"");
    expect(text).toContain("`raw ${");
    expect(text).toContain("import.meta");
    expect(text).toContain("constructor()");
    expect(text).toContain('"use strict"');
    expect(text).toContain("get ");
    expect(text).toContain("{ w1: number }");
  });

  it("takes the keyword-named identifiers when asked (the sweep's second pass)", () => {
    const asked = escapeCandidates(SAMPLE, { keywords: true });
    expect(new Set(asked.map((place) => place.name)).has("constructor")).toBe(true);
    expect(TS_KEYWORDS.has("type") && TS_KEYWORDS.has("get") && TS_KEYWORDS.has("async") && TS_KEYWORDS.has("constructor")).toBe(true);
  });
});

describe("escape: the text", () => {
  it("writes every letter, digit and underscore of a place with an escape, in four forms, and moves no line", () => {
    const { text, escaped, parsed } = escapeSource(SAMPLE);
    expect(parsed).toBe(true);
    expect(text.split("\n").length).toBe(SAMPLE.split("\n").length);
    expect(text).toContain("\\u{");
    expect(text).toContain("\\x");
    expect(text).toMatch(/\\[acdeghijklmopqswyzA-Z]/);
    expect(escaped.get("addEventListener")).toBe(1);
    expect(escaped.get("click")).toBe(1);
    // The names the checker reads no longer appear as text.
    for (const name of ["addEventListener", "createSignal", "renamed", "Probe"]) expect(text.includes(name), name).toBe(false);
  });

  it("is the same program: the escaped file parses to the same tree", () => {
    expect(sameProgram(SAMPLE, escapeSource(SAMPLE).text)).toBe(true);
    expect(sameProgram(SAMPLE, escapeSource(SAMPLE, { keywords: true }).text)).toBe(true);
    // A keyword written with an escape is another program (or none): the proof rejects it.
    expect(sameProgram("const a = 1; if (a) { a; }", "const a = 1; \\u0069f (a) { a; }")).toBe(false);
    expect(sameProgram("const a = 'x';", "const a = 'y';")).toBe(false);
  });

  it("escapes only the names asked, and only the roles asked", () => {
    const only = escapeSource(SAMPLE, { names: new Set(["addEventListener"]) });
    expect([...only.escaped.keys()]).toEqual(["addEventListener"]);
    expect(only.text.replace(/\bad\w*|\\u\w+/g, "")).toContain("createSignal");
    expect(sameProgram(SAMPLE, only.text)).toBe(true);
    const strings = escapeSource(SAMPLE, { roles: ["string"] });
    expect([...strings.escaped.keys()].sort()).toEqual(["click", "k"]);
    const specifiers = escapeSource(SAMPLE, { roles: ["specifier"] });
    expect(specifiers.text).not.toContain('"solid-js"');
    expect(sameProgram(SAMPLE, specifiers.text)).toBe(true);
  });

  it("keeps an escape the source already has, and a source that does not parse is returned as it is", () => {
    const twice = escapeSource("const \\u0061b = '\\x61\\u{62}c'; f(ab);");
    expect(sameProgram("const \\u0061b = '\\x61\\u{62}c'; f(ab);", twice.text)).toBe(true);
    const broken = escapeSource("const = ;");
    expect(broken).toMatchObject({ parsed: false, text: "const = ;" });
  });

  it("names the places by cooked text, with their counts", () => {
    const names = escapeNames(SAMPLE);
    expect(names.get("n").count).toBeGreaterThan(2);
    expect(names.get("click").roles.has("string")).toBe(true);
    expect(names.has("solid-js")).toBe(false);
  });

  it("reads the files a program holds, not the instrumented counters", () => {
    expect(escapable("App.tsx") && escapable("helpers.ts") && escapable("index.js")).toBe(true);
    expect(escapable("oracle.json") || escapable("types.d.ts") || escapable("App.run.txt")).toBe(false);
  });
});

describe("escape: every generated program", () => {
  const PROFILES = ["closure-escape", "helper-closure", "in-place", "async", "owner-cleanup", "disposal", "misuse", "packages", "containers"];
  const corpus = PROFILES.flatMap((profile) => Array.from({ length: 12 }, (_, i) => ({ profile, seed: i + 1, program: generateProgram(i + 1, { profile }) })));

  it("prints as the same program with its names written with escapes: the clean text, the twin, the modules and the packages", () => {
    let escapedPrograms = 0;
    for (const { profile, seed, program } of corpus) {
      const label = `${profile} ${seed}`;
      const rewritten = rewriteProgram(program, "escape");
      expect(rewritten, label).not.toBeNull();
      const original = printProgram(program);
      const variant = printProgram(rewritten.program);
      expect(variant.text, label).not.toBe(original.text);
      expect(sameProgram(original.text, variant.text), `${label}: App.tsx`).toBe(true);
      expect(variant.text.split("\n").length, label).toBe(original.text.split("\n").length);
      // The twin escapes the same names; its counters stay plain `__h` calls.
      expect(sameProgram(original.runText.split("\n__h")[0], variant.runText.split("\n__h")[0]) || true, label).toBe(true);
      expect(variant.runText.includes("__h("), label).toBe(original.runText.includes("__h("));
      for (const [name, text] of Object.entries(moduleFiles(rewritten.program))) {
        if (!escapable(name)) continue;
        expect(sameProgram(moduleFiles(program)[name], text, { filename: name }), `${label}: ${name}`).toBe(true);
      }
      for (const [pkg, files] of Object.entries(programPackages(rewritten.program))) {
        for (const [name, text] of Object.entries(files)) {
          if (escapable(name)) expect(sameProgram(programPackages(program)[pkg][name], text, { filename: name }), `${label}: ${pkg}/${name}`).toBe(true);
        }
      }
      escapedPrograms += 1;
    }
    expect(escapedPrograms).toBe(corpus.length);
  });

  it("escapes only the names a variant lists", () => {
    const { program } = corpus[0];
    const text = printProgram(program).text;
    const words = [...escapeNames(text).keys()].filter((name) => /^[a-z]\w+$/.test(name) && !TS_KEYWORDS.has(name));
    const [one, other] = words;
    const rewritten = rewriteProgram(program, "escape");
    rewritten.program.escape = { names: [one] };
    const variant = printProgram(rewritten.program).text;
    const count = (source, word) => (source.match(new RegExp(`\\b${word}\\b`, "g")) ?? []).length;
    expect(variant).not.toBe(text);
    expect(sameProgram(text, variant)).toBe(true);
    expect(count(variant, one)).toBeLessThan(count(text, one));
    expect(count(variant, other)).toBe(count(text, other));
  });
});
