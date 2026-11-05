import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_FAST_INSTALL, PINNED_SOLID, installedVersions, pinnedProblems, runFast } from "./fast.mjs";

const all = Object.fromEntries(["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics", "@solidjs/compiler", "@solidjs/babel-plugin"].map((name) => [name, PINNED_SOLID]));

describe("pinned install", () => {
  it("pins rc.17 (invariant 7: never a newer prerelease)", () => {
    expect(PINNED_SOLID).toBe("2.0.0-rc.21");
    expect(pinnedProblems(all)).toEqual([]);
  });
  it("names every package that is missing or not the pin", () => {
    const problems = pinnedProblems({ ...all, "@solidjs/compiler": "2.0.0-rc.18", "solid-js": undefined });
    expect(problems).toEqual(["solid-js is not installed", "@solidjs/compiler is 2.0.0-rc.18, not 2.0.0-rc.21"]);
  });
  it("reads versions from node_modules and refuses a runFast on an install that is not the pin", async () => {
    const install = mkdtempSync(join(tmpdir(), "fuzz-fast-"));
    mkdirSync(join(install, "node_modules/solid-js"), { recursive: true });
    writeFileSync(join(install, "node_modules/solid-js/package.json"), JSON.stringify({ version: "2.0.0-rc.18" }));
    expect(installedVersions(install)["solid-js"]).toBe("2.0.0-rc.18");
    await expect(runFast([], { install })).rejects.toThrow(/pinned install.*solid-js is 2.0.0-rc.18/);
  });
  it("refuses an unsafe case id before writing anything", async () => {
    await expect(runFast([{ id: "../x", dir: "/tmp" }], { install: "/nonexistent" })).rejects.toThrow(/unsafe case id/);
  });
});

// Fuzz audit run 41696018194: the runner's first click made a render effect's apply throw PRIMITIVE_IN_EFFECT_CALLBACK (an
// error-severity diagnostic, thrown at its site), the `errored` mount's boundary caught it and showed its fallback, and the plan's
// click on the same button found nothing. Chrome's halt marker did not fire (no REACTIVITY_HALTED, no uncaught error), so its step
// waited 12 s and failed in `execution` while this runner went on with `failure: null`. Both now read the step as halted.
const fastInstalled = existsSync(join(DEFAULT_FAST_INSTALL, "node_modules/vitest/dist/node.js"));
describe.skipIf(!fastInstalled)("halted steps", () => {
  const scratch = mkdtempSync(join(tmpdir(), "fuzz-fast-halt-"));
  const writeCase = (id, source) => {
    const dir = join(scratch, id);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "App.tsx"), source);
    writeFileSync(join(dir, "sites.json"), JSON.stringify({ plan: { clicks: ["b1"] }, sites: [] }));
    return { id, dir };
  };
  const program = (apply) => `import { Errored, createRenderEffect, createSignal } from "solid-js";
function C1() {
  const [t, setT] = createSignal(1);
  createRenderEffect(() => t(), (v) => { if (v === 1) return; ${apply} });
  return <div><button data-act="b1" onClick={() => setT(2)}>go</button></div>;
}
export function P1() { return <div><Errored fallback={<p>e</p>}><C1 /></Errored></div>; }
`;
  it("is halted when an Errored boundary caught an error-severity diagnostic's throw and the step's element is gone", async () => {
    const records = await runFast([writeCase("halt-caught", program("createRenderEffect(() => 0, () => {});"))]);
    const record = records.get("halt-caught");
    expect(record.diagnostics.map((d) => d.code)).toContain("PRIMITIVE_IN_EFFECT_CALLBACK");
    expect(record.failure).toMatchObject({ phase: "halted" });
  }, 120000);
  it("is not halted by a missing element alone (a plain throw the boundary caught reports no diagnostic)", async () => {
    const records = await runFast([writeCase("halt-plain", program('throw new Error("plain");'))]);
    const record = records.get("halt-plain");
    expect(record.diagnostics).toEqual([]);
    expect(record.failure).toBeNull();
    expect(record.errors.map((e) => e.kind)).toContain("plan");
  }, 120000);
});
