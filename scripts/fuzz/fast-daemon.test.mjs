import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { createFastDaemon } from "./fast-daemon.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";

const installed = existsSync(join(DEFAULT_FAST_INSTALL, "node_modules/vitest/dist/node.js"));
const scratch = mkdtempSync(join(tmpdir(), "fuzz-daemon-"));
const writeCase = (id, source, sites, clicks = []) => {
  const dir = join(scratch, id);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "App.tsx"), source);
  writeFileSync(join(dir, "sites.json"), JSON.stringify({ plan: { clicks }, sites }));
  return { id, dir };
};
const view = { id: "s1", witness: { selector: '[data-view="v1"]', text: "-" } };

// (The runner clicks every element twice and then the plan's button: the counter ends at 3.)
// Program A leaves state everywhere a program can: a global, the document, a prototype, a module-level signal runtime
// that has been halted by a throw. Program B reads them all and must find a clean world.
const leaver = writeCase("iso-a", `import { createMemo, createSignal } from "solid-js";
(globalThis as any).__leak = 1;
document.body.dataset.leak = "set";
(Element.prototype as any).__protoLeak = true;
export function C1() {
  const [n, setN] = createSignal(0);
  const m = createMemo(() => { if (n() === 0) throw new Error("A halts reactivity"); return n(); });
  return <p data-view="v1">{m()}</p>;
}
`, [view]);
const reader = writeCase("iso-b", `import { createSignal } from "solid-js";
const seen = [String((globalThis as any).__leak), String(document.body.dataset.leak), String((Element.prototype as any).__protoLeak)].join("/");
export function C1() {
  const [n, setN] = createSignal(0);
  return <div><p data-view="v1">{seen}</p><p data-view="v2">{n()}</p><button data-act="b1" onClick={() => setN(n() + 1)}>go</button></div>;
}
`, [{ id: "s1", witness: { selector: '[data-view="v1"]', text: "undefined/undefined/undefined" } }, { id: "s2", witness: { selector: '[data-view="v2"]', text: "3" } }], ["b1"]);

const shape = (record) => ({
  failure: record.failure, diagnostics: record.diagnostics.map((d) => [d.code, d.positions.map((p) => p.line)]),
  errors: record.errors.map((e) => [e.kind, e.message]), witnesses: record.witnesses.map((w) => [w.siteId, w.actual, w.reached]),
});

describe.skipIf(!installed)("the long-lived fast host", () => {
  const daemon = createFastDaemon({ workers: 2 });
  afterAll(() => daemon.close());

  it("program k+1 sees nothing program k left behind (globals, document, prototypes, a halted signal runtime)", async () => {
    const first = await daemon.run([leaver]);
    // The leaver did leave its marks: it halted its own reactivity.
    expect(first.get("iso-a").diagnostics.map((d) => d.code)).toContain("REACTIVITY_HALTED");
    // Sequentially, and in one batch with the leaver, the reader gets a clean world each time.
    for (const batch of [[reader], [leaver, reader], [reader, leaver]]) {
      const records = await daemon.run(batch);
      const record = records.get("iso-b");
      expect(record.failure).toBeNull();
      expect(record.witnesses.map((w) => w.actual)).toEqual(["undefined/undefined/undefined", "3"]);
      expect(record.diagnostics.some((d) => d.code === "REACTIVITY_HALTED")).toBe(false);
    }
  }, 120000);

  it("gives the same records as a cold Vitest process, request after request (ids reused with the same content)", async () => {
    const cold = await runFast([leaver, reader], { workers: 2 });
    for (let round = 0; round < 2; round += 1) {
      const warm = await daemon.run([leaver, reader]);
      for (const id of ["iso-a", "iso-b"]) expect(shape(warm.get(id)), `${id} round ${round}`).toEqual(shape(cold.get(id)));
    }
  }, 120000);

  it("a reused id with different content runs the new content", async () => {
    const one = writeCase("same-id", `export function C1() { return <p data-view="v1">first</p>; }\n`, [{ id: "s1", witness: { selector: '[data-view="v1"]', text: "first" } }]);
    expect((await daemon.run([one])).get("same-id").witnesses[0].actual).toBe("first");
    const two = writeCase("same-id", `export function C1() { return <p data-view="v1">second</p>; }\n`, [{ id: "s1", witness: { selector: '[data-view="v1"]', text: "second" } }]);
    expect((await daemon.run([two])).get("same-id").witnesses[0].actual).toBe("second");
  }, 120000);

  it("refuses an unsafe id and serves overlapping requests in order", async () => {
    await expect(daemon.run([{ id: "../x", dir: "/tmp" }])).rejects.toThrow(/unsafe case id/);
    const [a, b] = await Promise.all([daemon.run([reader]), daemon.run([leaver])]);
    expect(a.get("iso-b").witnesses[0].actual).toBe("undefined/undefined/undefined");
    expect(b.has("iso-a")).toBe(true);
  }, 120000);
});
