import { describe, expect, it } from "vitest";
import { cleanupEvidence } from "./disposal.mjs";
import { SEEDS } from "./disposal-seeds.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { validate } from "./model.mjs";
import { printProgram } from "./print.mjs";
import { stripInstrumentation } from "./hits.mjs";

const disposal = (seed, plan) => generateProgram(seed, { profile: "disposal", disposalPlan: plan });
const seeds = Array.from({ length: 400 }, (_, i) => i + 1);

describe("the disposal profile", () => {
  it("is deterministic and only produces valid compositions", () => {
    for (const seed of seeds) {
      expect(validate(disposal(seed)), `seed ${seed}`).toEqual([]);
      expect(printProgram(disposal(seed)).text).toBe(printProgram(disposal(seed)).text);
    }
  });

  it("covers every write kind, host, cleanup form and wrapper", () => {
    const shapes = seeds.map((seed) => disposal(seed).components[0].shape);
    const seen = (key) => new Set(shapes.flatMap((shape) => [shape[key]].flat()));
    expect(seen("via")).toEqual(new Set(["signal", "store", "refresh", "action"]));
    expect(seen("where")).toEqual(new Set([undefined, "show", "memo", "effect", "rootd", "body"]));
    expect(seen("handleMode")).toEqual(new Set(["none", "discard", "keep", "call-now", "call-later"]));
    expect(seen("control")).toEqual(new Set([undefined, "control-body", "control-handler"]));
    expect(seen("wrappers").size).toBe(21);
  });

  it("prints the write once, on its own line, with the counters stripping back to the clean text", () => {
    for (const seed of seeds.slice(0, 150)) {
      const built = buildCase(disposal(seed), { id: `d${seed}` });
      expect(built.sites).toHaveLength(1);
      expect(built.neutral, `seed ${seed}`).toBe(true);
      expect(stripInstrumentation(built.runText)).toBe(built.text);
      const line = built.text.split("\n")[built.sites[0].line - 1];
      expect(line).toMatch(/set[A-Z]\w*\(\d+\)|set[A-Z]\w*\(\(s\) => \{ s\.v = \d+; \}\)|refresh\(m\d+\)|a\d+\(\)/);
    }
  });

  it("every pinned shape validates and prints the construct it is named for", () => {
    const texts = SEEDS.map((seed, index) => {
      const program = disposal(100 + index, seed.plan);
      expect(validate(program), seed.name).toEqual([]);
      return printProgram(program).text;
    });
    const [p1, , , awaitShape, later, now, rerun, teardown, action, store, refreshMemo, controlBody, controlHandler] = texts;
    expect(p1).toMatch(/const owner = getOwner\(\);\s+onCleanup\(\(\) => \{\s+runWithOwner\(owner, \(\) => \{\s+setW1\(17\);/);
    expect(awaitShape).toMatch(/async \(\) => \{\s+await Promise\.resolve\(\);\s+runWithOwner\(owner/);
    expect(later).toMatch(/const h1 = onCleanup\(/);
    expect(later).toMatch(/return h1;/);
    expect(later).toMatch(/r1\(\);/);
    expect(now).toMatch(/h1\(\);/);
    expect(rerun).toMatch(/createEffect\(\(\) => \{\s+t1\(\);\s+return 1;\s+\}, \(v\) => \{\s+return \(\) => \{/);
    expect(teardown).toMatch(/<Show when=\{o1\(\)\}>/);
    expect(action).toMatch(/action\(function\* \(\) \{ setW1\(17\); \}\)/);
    expect(store).toMatch(/createStore\(\{ v: 0 \}\)/);
    expect(store).toMatch(/createRoot\(\(dispose\) => \{/);
    expect(refreshMemo).toMatch(/refresh\(m1\)/);
    expect(controlBody).toMatch(/^\s+setW1\(17\);$/m);
    expect(controlHandler).toMatch(/onClick=\{\(\) => \{\s+setW1\(17\);/);
  });

  it("rejects an action whose target signal was removed (the shrinker's edit)", () => {
    const program = disposal(1, { ...SEEDS[8].plan });
    expect(validate(program)).toEqual([]);
    const broken = structuredClone(program);
    broken.components[0].signals = broken.components[0].signals.filter((sig) => sig.kind === "action");
    expect(validate(broken).some((message) => message.includes("not in scope"))).toBe(true);
  });

  it("finds the cleanup callback's entry counter in the ledger", () => {
    const program = disposal(1, SEEDS[0].plan);
    const built = buildCase(program, { id: "d1" });
    const hits = Object.fromEntries(built.ledger.map((row) => [row.hit, 0]));
    const evidence = cleanupEvidence(program, built.ledger, hits);
    expect(evidence.entered).toBe(0);
    hits[evidence.hit] = 2;
    expect(cleanupEvidence(program, built.ledger, hits).entered).toBe(2);
  });
});
