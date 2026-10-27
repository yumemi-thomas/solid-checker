import { readFileSync } from "node:fs";
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
