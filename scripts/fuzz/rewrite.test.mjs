import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { materializeForChecker } from "./checker.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { siteMap } from "./equivalence.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { call, component, decl, expr, fn, helper, iife, program as makeProgram, read, ref, ret, signal, write, validate, walk } from "./model.mjs";
import { printProgram } from "./print.mjs";
import { REWRITES, REWRITE_KINDS, SITE_TAG, applicableKinds, rewriteProgram, tagSites } from "./rewrite.mjs";
import { resolveTsc, typecheck } from "./typecheck.mjs";

const PROFILES = ["closure-escape", "helper-closure", "in-place", "async", "owner-cleanup", "disposal", "misuse", "packages", "containers"];
const seeds = Array.from({ length: 40 }, (_, i) => i + 1);
const corpus = PROFILES.flatMap((profile) => seeds.map((seed) => ({ profile, seed, program: generateProgram(seed, { profile }) })));

describe("rewrites: the registry", () => {
  it("states a soundness argument for every kind, and none depends on timing, owners or tracking", () => {
    expect(REWRITE_KINDS.length).toBeGreaterThanOrEqual(10);
    for (const kind of REWRITE_KINDS) expect(REWRITES[kind].sound.length, kind).toBeGreaterThan(40);
    for (const forbidden of ["untrack", "effect", "createRoot", "component body"]) for (const kind of REWRITE_KINDS) expect(kind).not.toBe(forbidden);
  });

  it("applies every kind somewhere in the corpus", () => {
    const seen = new Set();
    for (const { program } of corpus) for (const kind of applicableKinds(program)) seen.add(kind);
    expect([...seen].sort()).toEqual([...REWRITE_KINDS].sort());
  });
});

describe("rewrites: each keeps the sites and the site map", () => {
  for (const kind of REWRITE_KINDS) {
    it(`${kind}: same sites, tagged with their original ids, still neutral and valid`, () => {
      let applied = 0;
      for (const { profile, seed, program } of corpus) {
        const rewritten = rewriteProgram(program, kind);
        if (!rewritten) continue;
        applied += 1;
        const original = printProgram(program);
        const variant = printProgram(rewritten.program);
        const label = `${profile} seed ${seed}`;
        expect(variant.sites.length, label).toBe(original.sites.length);
        // The site map is the tag: a bijection onto the original ids, between sites of the same kind and signal.
        const map = siteMap(rewritten.program, variant.sites);
        expect([...map.values()].sort(), label).toEqual(original.sites.map((site) => site.id).sort());
        for (const site of variant.sites) {
          const origin = original.sites.find((candidate) => candidate.id === map.get(site.id));
          expect([site.kind, site.sig, site.op, site.code], label).toEqual([origin.kind, origin.sig, origin.op, origin.code]);
          expect(site.line, label).toBeGreaterThan(0);
        }
        expect(variant.text, label).not.toBe(original.text);
        expect(stripInstrumentation(variant.runText), label).toBe(variant.text);
        expect(validate(rewritten.program), label).toEqual([]);
      }
      expect(applied, kind).toBeGreaterThan(0);
    });
  }

  it("does not change the original", () => {
    const program = generateProgram(3, { profile: "in-place" });
    const before = JSON.stringify(program);
    for (const kind of REWRITE_KINDS) rewriteProgram(program, kind);
    expect(JSON.stringify(program)).toBe(before);
    expect([...walk(program)].some(({ node }) => SITE_TAG in node)).toBe(false);
    expect([...walk(tagSites(program))].filter(({ node }) => SITE_TAG in node).length).toBe(printProgram(program).sites.length);
  });

  it("is a function of the program and its choices", () => {
    const program = generateProgram(9, { profile: "closure-escape" });
    for (const kind of applicableKinds(program)) expect(printProgram(rewriteProgram(program, kind).program).text).toBe(printProgram(rewriteProgram(program, kind).program).text);
  });
});

// A small program in the model's own constructors, so each printed shape is pinned.
const small = () => makeProgram({
  seed: 1, helpers: [helper("h1", "call")], moduleSignals: [],
  components: [component("C1", [signal("w1", 0, "probe"), signal("r1", 1)], [
    decl("f1", fn([expr(write("w1", 5, { site: true })), ret(read("r1", { site: true }))])),
    expr(call(ref("f1"))),
  ], [])],
});

describe("rewrites: the printed shapes", () => {
  const text = (kind) => printProgram(rewriteProgram(small(), kind).program).text;
  const body = (kind) => text(kind).split("\n").filter((line) => /w1|r1/i.test(line) && !/createSignal/.test(line)).map((line) => line.trim());
  it("wrappers keep a one-line statement on its line", () => {
    expect(body("block")).toContain("{ setW1(5); }");
    expect(body("iife")).toContain("(() => { setW1(5); })();");
    expect(body("if-true")).toContain("if (true) { setW1(5); }");
    expect(body("try-finally")).toContain("try { setW1(5); } finally {}");
    expect(text("outline")).toMatch(/function eq\d\(\) \{ setW1\(5\); \} eq\d\(\);/); expect(text("outline")).toMatch(/const eq\d = \(\) => \{ f1\(\); \}; eq\d\(\);/);
  });
  it("extract binds the value of a statement and of a return", () => {
    const t = text("extract");
    expect(t).toMatch(/const eq\d = setW1\(5\);/);
    expect(t).toMatch(/const eq\d = r1\(\); return eq\d;/);
  });
  it("call forms rewrite names and accessors", () => {
    expect(text("callform")).toMatch(/\((f1|void 0, f1|f1 as typeof f1|true && f1)\)\(\)|f1\?\.\(\)/);
    expect(text("accessor-form")).toMatch(/\((setW1|void 0, setW1|setW1 as typeof setW1|true && setW1)\)\(5\)|setW1\?\.\(5\)|r1\?\.\(\)|\((r1|void 0, r1|r1 as typeof r1|true && r1)\)\(\)/);
  });
  it("declform and rename change the declaration only", () => {
    expect(text("declform")).toContain("function f1() {");
    expect(text("rename")).toContain("v_f1");
    expect(text("rename")).not.toMatch(/\bf1\b/);
  });
  it("inline splices an IIFE statement's body, and refuses one with a return", () => {
    const withIife = makeProgram({ seed: 1, components: [component("C1", [signal("w1", 0, "probe")], [expr(iife(fn([expr(write("w1", 5, { site: true }))])))], [])] });
    expect(printProgram(rewriteProgram(withIife, "inline").program).text).not.toContain("=> {");
    const returning = makeProgram({ seed: 1, components: [component("C1", [signal("w1", 0, "probe")], [expr(iife(fn([ret(write("w1", 5, { site: true }))])))], [])] });
    expect(rewriteProgram(returning, "inline")).toBeNull();
  });
  it("reorder never moves a closure across a statement that uses it", () => {
    expect(rewriteProgram(small(), "reorder")).toBeNull();
    const independent = makeProgram({ seed: 1, components: [component("C1", [signal("w1", 0, "probe")], [decl("f1", fn([])), expr(write("w1", 5, { site: true }))], [])] });
    expect(printProgram(rewriteProgram(independent, "reorder").program).text).toMatch(/setW1\(5\);\n\s+const f1/);
  });
  it("dead-decl adds a declaration nothing reads", () => {
    expect(text("dead-decl")).toMatch(/(const eq1 = 7;|const eq1 = \(\) => \{\};|function eq1\(\) \{\})/);
  });
  it("fresh names never collide with the generator's", () => {
    for (const { program } of corpus) {
      const names = new Set([...walk(program)].flatMap(({ node }) => (node.k === "decl" || node.k === "ref" || node.k === "helper" ? [node.name] : [])));
      expect([...names].filter((name) => /^(eq\d+|v_)/.test(name))).toEqual([]);
    }
  });
});

// The rewritten programs type-check against the published typings with tsrs; skipped where tsrs or the install is not there.
const TSC = resolveTsc();
const typed = existsSync(TSC) && existsSync(join(DEFAULT_ORACLE_INSTALL, "node_modules/solid-js/package.json"));
describe.skipIf(!typed)("rewrites type-check with tsrs", () => {
  it("every kind, on a sample of each profile", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "fuzz-rewrite-"));
    const failures = [];
    try {
      for (const kind of REWRITE_KINDS) {
        const sample = corpus.filter((_, index) => index % 9 === 0);
        for (const { profile, seed, program } of sample) {
          const original = buildCase(program, { id: `o-${seed}` });
          const baseDir = materializeForChecker(original, DEFAULT_ORACLE_INSTALL, `eqt-o-${profile}-${seed}`);
          const baseline = await typecheck(baseDir, { tsc: TSC });
          rmSync(baseDir, { recursive: true, force: true });
          if (!baseline.ok) continue; // the generator's own type error says nothing about the rewrite
          const rewritten = rewriteProgram(program, kind);
          if (!rewritten) continue;
          const built = buildCase(rewritten.program, { id: `eqt-${kind}-${profile}-${seed}` });
          const dir = materializeForChecker(built, DEFAULT_ORACLE_INSTALL, built.id);
          const answer = await typecheck(dir, { tsc: TSC });
          rmSync(dir, { recursive: true, force: true });
          if (!answer.ok) failures.push(`${kind} ${profile} ${seed}: ${answer.errors[0]}`);
        }
      }
    } finally { rmSync(tmp, { recursive: true, force: true }); }
    expect(failures).toEqual([]);
  }, 300000);
});
