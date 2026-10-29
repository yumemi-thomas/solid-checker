import { describe, expect, it } from "vitest";
import { OWNER_PROFILES, generateProgram } from "./generate.mjs";
import { buildCase, scenarioSteps } from "./emit.mjs";
import { haltable } from "./hits.mjs";
import { validate } from "./model.mjs";
import { printProgram } from "./print.mjs";

const SEEDS = Array.from({ length: 600 }, (_, i) => i + 1);

describe("generateProgram", () => {
  it("is a pure function of the seed", () => {
    for (const seed of [1, 2, 99, 12345]) {
      expect(generateProgram(seed)).toEqual(generateProgram(seed));
      expect(printProgram(generateProgram(seed)).text).toBe(printProgram(generateProgram(seed)).text);
    }
    expect(printProgram(generateProgram(1)).text).not.toBe(printProgram(generateProgram(2)).text.replace("C2", "C1"));
  });

  it("only produces valid compositions", () => {
    for (const seed of SEEDS) expect(validate(generateProgram(seed)), `seed ${seed}`).toEqual([]);
  });

  it("produces 20-60 line programs on average, never huge", () => {
    const sizes = SEEDS.map((seed) => printProgram(generateProgram(seed)).text.split("\n").length);
    const mean = sizes.reduce((a, b) => a + b, 0) / sizes.length;
    expect(mean).toBeGreaterThan(18);
    expect(mean).toBeLessThan(60);
    expect(Math.max(...sizes)).toBeLessThan(150);
  });

  it("records every site on the line its call is written on, with a witness", () => {
    for (const seed of SEEDS.slice(0, 150)) {
      const { text, sites } = printProgram(generateProgram(seed));
      const lines = text.split("\n");
      expect(text).not.toMatch(/[\u0001\u0002]/);
      expect(sites.length, `seed ${seed}`).toBeGreaterThan(0);
      for (const site of sites) {
        expect(Number.isInteger(site.line), `seed ${seed} ${site.id}`).toBe(true);
        expect(lines[site.line - 1]).toContain(site.kind === "write" ? `set${site.sig[0].toUpperCase()}${site.sig.slice(1)}(${site.value})` : `${site.sig}()`);
        expect(site.witness?.selector).toBeTruthy();
      }
    }
  });

  it("covers the construct space, including the ADR 0452 false-positive family", () => {
    const texts = SEEDS.map((seed) => printProgram(generateProgram(seed)).text);
    const seen = (pattern) => texts.filter((text) => pattern.test(text)).length;
    const required = {
      "named closure returned from a memo": /createMemo\(\(\) => \{[\s\S]*?const f\d = [\s\S]*?return f\d;/,
      "a closure stored in an object": /return \{ f(?:: f\d)? \};/,
      "plain default": /p0: (?:number|unknown|void) = /,
      "destructured default": /\{ p0 = [\s\S]*?\} = \{\}/,
      "IIFE inside": /\(\(\) => \{/,
      "forEach": /\[1\]\.forEach\(/,
      "map": /\[1\]\.map\(/,
      "helper returning a closure": /const g = \(x: unknown = cb\(\)\) => x;/,
      "helper call": /\bh\d\(/,
      "called later from a click": /m\d\(\)\(\)|u\d\(\)|m\d\(\)\.f\(\)/,
      "effect": /createEffect\(/,
      "untrack": /untrack\(/,
      "createRoot": /createRoot\(/,
      "runWithOwner": /runWithOwner\(/,
      "onSettled": /onSettled\(/,
      "onCleanup": /onCleanup\(/,
      "setTimeout": /setTimeout\(/,
      "promise": /Promise\.resolve\(\)\.then\(/,
      "await": /await Promise\.resolve\(\);/,
      "Show": /<Show /,
      "For": /<For /,
      "attribute position": /data-attr=/,
      "handler position": /onClick=/,
      "read": /createSignal\(10\d\d\)/,
    };
    for (const [name, pattern] of Object.entries(required)) expect(seen(pattern), name).toBeGreaterThan(0);
  });

  it("body-call: the component body calls what a memo returned (m()())", () => {
    const texts = SEEDS.slice(0, 100).map((seed) => printProgram(generateProgram(seed, { profile: "body-call" })).text);
    expect(texts.filter((text) => /^  (?:const n\d = )?m\d\(\)(?:\(\)|\.f\(\))/m.test(text)).length).toBeGreaterThan(5);
    for (const seed of SEEDS.slice(0, 100)) expect(validate(generateProgram(seed, { profile: "body-call" })), `seed ${seed}`).toEqual([]);
  });

  it("honours the profile and component count options", () => {
    const program = generateProgram(3, { profile: "default-param", components: 3 });
    expect(program.components).toHaveLength(3);
    expect(generateProgram(3).components).toHaveLength(1);
    expect(printProgram(program).text).toMatch(/p0/);
  });
});

describe("owner profiles (phase C)", () => {
  it("only produce valid compositions with an owner site, never fall back to the plain write", () => {
    for (const profile of Object.keys(OWNER_PROFILES)) {
      for (const seed of SEEDS.slice(0, 200)) {
        const program = generateProgram(seed, { profile });
        expect(validate(program), `${profile} seed ${seed}`).toEqual([]);
        expect(JSON.stringify(program), `${profile} seed ${seed}`).toContain('"k":"own"');
      }
    }
  });

  it("cover every op, owned and ownerless hosts, and print the Solid call", () => {
    const texts = SEEDS.slice(0, 200).map((seed) => printProgram(generateProgram(seed, { profile: "owner" })).text);
    const seen = (pattern) => texts.filter((text) => pattern.test(text)).length;
    for (const [name, pattern] of Object.entries({
      cleanup: /onCleanup\(/, effect: /createEffect\(\(\) => \{\n\s*return 1;/, settled: /onSettled\(/, context: /useContext\(Ctx\)/,
      "a context is declared once": /^const Ctx = createContext\(7\);$/m, "in a handler": /onClick=\{\(\) => \{[\s\S]*?(?:onCleanup|onSettled|createEffect|useContext)/,
      "in a timer": /setTimeout\(/, "under runWithOwner(null": /runWithOwner\(null/, "in a memo": /createMemo\(/,
    })) expect(seen(pattern), name).toBeGreaterThan(0);
    for (const text of texts) if (/useContext/.test(text)) expect(text).toContain("const Ctx = createContext(7);");
  });

  it("a named owner profile fixes the op; the other profiles draw no owner site", () => {
    for (const seed of SEEDS.slice(0, 30)) {
      expect(JSON.stringify(generateProgram(seed, { profile: "owner-cleanup" }))).toContain('"op":"cleanup"');
      expect(JSON.stringify(generateProgram(seed, { profile: "owner-cleanup" }))).not.toMatch(/"op":"(effect|settled|context)"/);
    }
    // (Checked once against the commit before the owner profiles, 8 profiles x 400 seeds: the text and the twin were byte-equal.)
    for (const profile of [undefined, "body-call", "mixed"]) expect(JSON.stringify(generateProgram(3, { profile }))).not.toContain('"k":"own"');
  });
});

describe("buildCase", () => {
  it("emits an oracle case: App.tsx, oracle.json steps, program.json, sites.json", () => {
    const built = buildCase(generateProgram(2802362291), { id: "fuzz-x" });
    expect(Object.keys(built.files).sort()).toEqual(["App.run.txt", "App.tsx", "oracle.json", "oracle.run.json", "program.json", "sites.json"]);
    const oracle = JSON.parse(built.files["oracle.json"]);
    expect(oracle.steps.length).toBeLessThanOrEqual(60);
    for (const step of oracle.steps) expect(["click", "wait-for-selector"]).toContain(step.action);
    expect(JSON.parse(built.files["sites.json"]).sites).toEqual(JSON.parse(JSON.stringify(built.sites)));
    // The tree round-trips: reading program.json prints the same file.
    expect(printProgram(JSON.parse(built.files["program.json"])).text).toBe(built.text);
  });

  it("lists plan clicks first and one read per distinct witness selector", () => {
    const steps = scenarioSteps({ clicks: ["b1", "b2"] }, [
      { witness: { selector: "[data-probe=\"w1\"]" } }, { witness: { selector: "[data-probe=\"w1\"]" } }, { witness: null },
    ]);
    // Each is haltable (hits.mjs): after REACTIVITY_HALTED a missing element resolves to the oracle's halt marker at once.
    expect(steps.map((s) => `${s.action} ${s.selector}`)).toEqual([
      `click ${haltable('[data-act="b1"]')}`, `click ${haltable('[data-act="b2"]')}`, `wait-for-selector ${haltable('[data-probe="w1"]')}`,
    ]);
    expect(haltable('[data-act="b1"]')).toBe('[data-act="b1"], body:not(:has([data-act="b1"])) > #sc-halted');
  });

  it("types a plan's edits before its clicks, into the plain selector", () => {
    const steps = scenarioSteps({ clicks: ["b1"], edits: [{ selector: "#i", text: "typed" }] }, []);
    expect(steps).toEqual([{ action: "fill", selector: "#i", text: "typed" }, { action: "click", selector: haltable('[data-act="b1"]') }]);
  });

  it("asks the oracle for the halt marker in both scenarios", () => {
    const built = buildCase(generateProgram(2802362291), { id: "fuzz-x" });
    expect(JSON.parse(built.files["oracle.json"]).haltMarker).toBe(true);
    expect(JSON.parse(built.files["oracle.run.json"]).haltMarker).toBe(true);
  });
});
