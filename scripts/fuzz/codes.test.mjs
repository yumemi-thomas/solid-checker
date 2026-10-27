import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { codeOfThrown, codeRules, codesForRule, ruleHasRuntimeCounterpart, thrownCodes, withholdingRules } from "./codes.mjs";

const oracle = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../runtime-oracle.mjs"), "utf8");
/** The source text of a top-level `const name = [...]` or `{...}` literal in the oracle. */
const literal = (name) => {
  const open = `const ${name} = `;
  const start = oracle.indexOf(open) + open.length;
  const close = oracle[start] === "[" ? "\n];" : "\n};";
  return oracle.slice(start, oracle.indexOf(close, start) + close.length - 1);
};

describe("tables copied from scripts/runtime-oracle.mjs", () => {
  it("codeRules matches the oracle's", () => {
    // eslint-disable-next-line no-new-func
    expect(codeRules).toEqual(new Function(`return (${literal("codeRules")});`)());
  });
  it("thrownCodes matches the oracle's patterns and codes", () => {
    // eslint-disable-next-line no-new-func
    const theirs = new Function(`return (${literal("thrownCodes")});`)();
    expect(thrownCodes.map(([pattern, code]) => [String(pattern), code])).toEqual(theirs.map(([pattern, code]) => [String(pattern), code]));
  });
  it("withholdingRules matches the oracle's", () => {
    expect(oracle).toContain(`const withholdingRules = ${JSON.stringify(withholdingRules)};`);
  });
});

const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const distDir = join(repo, "scripts/runtime-oracle-install/node_modules/@solidjs/signals/dist");

describe("every claimed code and rule is real", () => {
  it("each rule a code names has a rule page (docs/rules/<rule>.md)", () => {
    for (const [code, rules] of Object.entries(codeRules)) {
      for (const rule of rules) expect(existsSync(join(repo, "docs/rules", `${rule}.md`)), `${code} -> ${rule}`).toBe(true);
    }
  });
  it("each code is one the pinned dev build emits (skipped without the oracle install)", () => {
    if (!existsSync(distDir)) return;
    const dist = readdirSync(distDir).filter((name) => name.endsWith(".js")).map((name) => readFileSync(join(distDir, name), "utf8")).join("\n");
    // The codes the dev dist emits (`code: "X"`) or throws with a text `thrownCodes` names.
    const thrown = new Set(thrownCodes.map(([, code]) => code));
    for (const code of Object.keys(codeRules)) expect(dist.includes(`"${code}"`) || thrown.has(code), code).toBe(true);
  });
  it("STORE_WRITE_OUTSIDE_SETTER is claimed by SC2003, so a store write the checker misses is a false clean", () => {
    expect(codeRules.STORE_WRITE_OUTSIDE_SETTER).toEqual(["no-store-mutation-outside-setter"]);
    expect(codesForRule("no-store-mutation-outside-setter")).toEqual(["STORE_WRITE_OUTSIDE_SETTER"]);
    expect(ruleHasRuntimeCounterpart("no-store-mutation-outside-setter")).toBe(true);
    expect(readFileSync(join(repo, "docs/rules/no-store-mutation-outside-setter.md"), "utf8")).toMatch(/SC2003/);
  });
});

describe("lookups", () => {
  it("maps rules to codes and back", () => {
    expect(codesForRule("no-write-in-pure-computation")).toEqual(["REACTIVE_WRITE_IN_OWNED_SCOPE"]);
    expect(codesForRule("missing-owner").sort()).toEqual(["NO_OWNER_BOUNDARY", "NO_OWNER_CLEANUP", "NO_OWNER_CONTEXT", "NO_OWNER_EFFECT", "SETTLED_CLEANUP_UNOWNED"]);
    expect(ruleHasRuntimeCounterpart("strict-read-untracked")).toBe(true);
    expect(ruleHasRuntimeCounterpart("jsx-no-duplicate-props")).toBe(false);
  });
  it("names thrown dev-build errors", () => {
    expect(codeOfThrown("Cannot call until inside a reactive scope")).toBe("UNTIL_IN_REACTIVE_SCOPE");
    expect(codeOfThrown("[FLUSH_IN_ACTION] x")).toBe("FLUSH_IN_ACTION");
    expect(codeOfThrown("something else")).toBeNull();
    expect(codeOfThrown(undefined)).toBeNull();
  });
});
