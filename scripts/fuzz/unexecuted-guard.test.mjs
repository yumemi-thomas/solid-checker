// ADR 1018: a legal program's checker violation at a site no run reaches (legal.mjs outcome `unexecuted`) needs a look, in the audit, the smoke and
// every focus run (codes.mjs `rowProblem`), unless scripts/fuzz/unexecuted-known.json holds its shape as `explained` or `pending-fix` with an ADR.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { UNEXECUTED_KNOWN, rowProblem, unexecutedEntry, unexecutedViolation } from "./codes.mjs";
import { compare } from "./smoke.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const decisions = readFileSync(join(here, "../../docs/decisions.md"), "utf8");
const legal = (id, outcome = "unexecuted") => ({ id, outcome, detail: { findings: ["no-write-in-pure-computation/violation@6"] } });

describe("an unexecuted violation", () => {
  it("a new one, of a shape the ledger does not hold, needs a look", () => {
    expect(rowProblem(legal("lg-reactive-write-in-owned-scope-new-dead-shape-store-direct-none"))).toBe("unexecuted-violation");
    expect(unexecutedViolation(legal("lg-reactive-write-in-owned-scope-new-dead-shape-store-direct-none"))).toBe(true);
  });

  it("a ledgered one passes, and names its entry", () => {
    const row = legal("lg-reactive-write-in-owned-scope-gate-throw-store-destructured-helper");
    expect(rowProblem(row)).toBeNull();
    expect(unexecutedEntry(row)).toMatchObject({ kind: "pending-fix", adr: "ADR 1010" });
  });

  it("an entry removed from the ledger (its fix landed) makes the same row fail again", () => {
    const row = legal("lg-untracked-read-after-await-ob-ctor-new-memo-async-never-x-direct-none");
    const without = { entries: UNEXECUTED_KNOWN.entries.filter((entry) => entry.item !== "ob-async-never") };
    expect(rowProblem(row)).toBeNull();
    expect(rowProblem(row, { unexecutedKnown: without })).toBe("unexecuted-violation");
  });

  it("a misuse grid's `unexecuted` (the page halted before the site) is another outcome, and other outcomes are untouched", () => {
    expect(rowProblem({ id: "mx-reactive-write-in-owned-scope-body-signal-direct-none", outcome: "unexecuted" })).toBeNull();
    expect(rowProblem(legal("lg-x-new", "uncertifiable"))).toBeNull();
    expect(rowProblem(legal("lg-x-new", "false-positive"))).toBe("false-positive");
  });

  it("a smoke move into an unledgered unexecuted row is a problem move", () => {
    const before = [{ id: "lg-x-new-dead", outcome: "uncertifiable", detail: {} }];
    const after = [legal("lg-x-new-dead")];
    expect(compare(before, after, ["lg-x-new-dead"]).problems).toBe(1);
    const held = [{ ...legal("lg-reactive-write-in-owned-scope-gate-throw-store-destructured-helper") }];
    expect(compare([{ ...held[0], outcome: "uncertifiable" }], held, [held[0].id]).problems).toBe(0);
  });
});

describe("the ledger", () => {
  it("every entry names a legal grid, a kind, a pattern that compiles, an ADR of docs/decisions.md and a reason", () => {
    expect(UNEXECUTED_KNOWN.entries.length).toBeGreaterThan(0);
    for (const entry of UNEXECUTED_KNOWN.entries) {
      expect(entry.grid).toMatch(/^lg-/);
      expect(["explained", "pending-fix"]).toContain(entry.kind);
      expect(() => new RegExp(entry.idRegex)).not.toThrow();
      expect(entry.adr).toMatch(/^ADR \d{4}$/);
      expect(entry.why.length).toBeGreaterThan(20);
      expect(entry.item).toBeTruthy();
    }
  });

  it("a pending-fix entry names an ADR that is reserved for it (its number is in docs/decisions.md's ADR 1018 entry)", () => {
    const entry1018 = decisions.slice(decisions.indexOf('<a id="adr-1018"></a>'));
    expect(decisions).toContain('<a id="adr-1018"></a>');
    for (const entry of UNEXECUTED_KNOWN.entries.filter((e) => e.kind === "pending-fix")) expect(entry1018.slice(0, 4000)).toContain(entry.adr.replace("ADR ", ""));
  });

  it("the seed holds the Chrome-confirmed groups of the full audit at 78ddeb849 whose fix has not landed", () => {
    const ids = {
      "gate-throw": "lg-reactive-write-in-owned-scope-gate-throw-store-destructured-helper",
      "apply-void-value": "lg-sync-node-received-async-apply-void-value-untrack-memo-alias-iife",
      "ob-async-never": "lg-untracked-read-after-await-ob-getter-memo-async-never-x-direct-helper",
    };
    for (const [item, id] of Object.entries(ids)) expect(unexecutedEntry(legal(id))?.item).toBe(item);
  });

  it("a group whose fix landed is out of the ledger, so its shape fails again if it comes back (tbl-zero, ADR 1019)", () => {
    expect(rowProblem(legal("lg-action-called-in-owned-scope-tbl-map-foreach-zero-x-direct-none"))).toBe("unexecuted-violation");
  });
});
