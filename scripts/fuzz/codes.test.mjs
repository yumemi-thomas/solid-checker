import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { DISPOSITIONS, codeOfThrown, codePolicy, codeRules, codesForRule, consequenceCodes, isDefectCode, policyOf, ruleHasRuntimeCounterpart, severityRank, thrownCodes, withholdingRules } from "./codes.mjs";

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

// The drift test: every code the pinned dev dists emit is classified in the policy table.
const fastModules = join(repo, "scripts/fuzz/install/node_modules");
const distFiles = (dir) => (existsSync(dir) ? readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
  const path = join(dir, entry.name);
  if (entry.isDirectory()) return entry.name === "types" ? [] : distFiles(path);
  return entry.name.endsWith(".js") ? [path] : [];
}) : []);
/** Every `code: "X"` in the dev dists, with the severities found in the 400 characters after it. */
const emittedCodes = () => {
  const found = new Map();
  for (const pkg of ["@solidjs/signals", "@solidjs/web", "solid-js"]) {
    for (const file of distFiles(join(fastModules, pkg, "dist"))) {
      const source = readFileSync(file, "utf8");
      for (const match of source.matchAll(/code: "([A-Z][A-Z_]+)"/g)) {
        const severity = source.slice(match.index, match.index + 400).match(/severity: "(\w+)"/)?.[1];
        const entry = found.get(match[1]) ?? new Set();
        if (severity) entry.add(severity);
        found.set(match[1], entry);
      }
    }
  }
  return found;
};

describe("the policy table", () => {
  it("is well formed: a known disposition, a severity, rules only on claimed defects", () => {
    for (const [code, policy] of Object.entries(codePolicy)) {
      expect(DISPOSITIONS, code).toContain(policy.disposition);
      expect(["error", "warn", "info"], code).toContain(policy.severity);
      expect(policy.rules.length > 0, code).toBe(policy.disposition === "defect-claimed");
    }
  });
  it("classifies the ones the proposal lists", () => {
    expect(Object.values(codePolicy).filter((policy) => policy.disposition === "defect-claimed")).toHaveLength(23);
    for (const code of ["PRIMITIVE_IN_EFFECT_CALLBACK", "RUN_WITH_DISPOSED_OWNER", "PENDING_ASYNC_FORBIDDEN_SCOPE", "ASYNC_STORE_SETTER", "UNCOMMITTED_INPUT_OVERWRITTEN", "LOADING_ON_OUTSIDE_HOLD"]) {
      expect(policyOf(code).disposition, code).toBe("defect-unclaimed");
      expect(isDefectCode(code), code).toBe(true);
    }
    expect(consequenceCodes.sort()).toEqual(["INVARIANT_VIOLATION", "REACTIVITY_HALTED", "SETTLE_WALK_UNINITIALIZED_SOURCE"]);
    for (const code of ["HUGE_FAN_OUT", "HUGE_FAN_IN", "HEAD_TAG_INVALID", "INVALID_REFRESH_TARGET", "REACTIVITY_HALTED"]) expect(isDefectCode(code), code).toBe(false);
    expect(isDefectCode("NOT_A_CODE")).toBe(false);
  });
  it("outranks warnings with errors", () => {
    expect(severityRank("REACTIVE_WRITE_IN_OWNED_SCOPE")).toBeLessThan(severityRank("STRICT_READ_UNTRACKED"));
    expect(severityRank("NOT_A_CODE")).toBeGreaterThan(severityRank("HUGE_FAN_OUT"));
  });
  it("keeps STORE_WRITE_OUTSIDE_SETTER mapped to SC2003 (it was once missing, so the oracle reported no-rule)", () => {
    expect(codePolicy.STORE_WRITE_OUTSIDE_SETTER).toMatchObject({ rules: ["no-store-mutation-outside-setter"], disposition: "defect-claimed", severity: "warn" });
  });
  it("names every thrown code", () => {
    for (const [, code] of thrownCodes) expect(codePolicy[code], code).toBeDefined();
  });
  it("classifies every code the pinned rc.17 dev dists emit, with the severity they emit (skipped without the fast install)", () => {
    if (!existsSync(join(fastModules, "@solidjs/signals/package.json"))) return;
    for (const pkg of ["@solidjs/signals", "@solidjs/web", "solid-js"]) {
      const { version } = JSON.parse(readFileSync(join(fastModules, pkg, "package.json"), "utf8"));
      expect(version, pkg).toMatch(/^2\.0\.0-rc\.17$/);
    }
    const emitted = emittedCodes();
    expect(emitted.size).toBeGreaterThan(30);
    const unclassified = [...emitted.keys()].filter((code) => !codePolicy[code]);
    expect(unclassified, `unclassified codes: add them to codePolicy (scripts/fuzz/codes.mjs)`).toEqual([]);
    for (const [code, severities] of emitted) {
      if (severities.size) expect([...severities], `${code}: severity in the dist`).toContain(codePolicy[code].severity);
    }
  });
  it("lists no code that nothing emits (a stale entry), apart from the thrown ones", () => {
    if (!existsSync(join(fastModules, "@solidjs/signals/package.json"))) return;
    const emitted = emittedCodes();
    const thrown = new Set(thrownCodes.map(([, code]) => code));
    const dist = ["@solidjs/signals", "@solidjs/web", "solid-js"].flatMap((pkg) => distFiles(join(fastModules, pkg, "dist"))).map((file) => readFileSync(file, "utf8")).join("\n");
    for (const code of Object.keys(codePolicy)) expect(emitted.has(code) || thrown.has(code) || dist.includes(`"${code}"`), code).toBe(true);
  });
});
