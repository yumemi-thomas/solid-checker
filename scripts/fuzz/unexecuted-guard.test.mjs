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
// A held entry of the ledger's shape: every pending fix of the 78ddeb849 seed has landed, so the real ledger is empty and the hold is tested on this one.
const SYNTHETIC = { entries: [{ grid: "lg-recall", kind: "pending-fix", item: "synthetic-dead", idRegex: "^lg-[a-z-]+-synthetic-dead-", adr: "ADR 1018", why: "a synthetic held shape for the guard's own tests: no grid writes it" }] };
const legal = (id, outcome = "unexecuted") => ({ id, outcome, detail: { findings: ["no-write-in-pure-computation/violation@6"] } });

describe("an unexecuted violation", () => {
  it("a new one, of a shape the ledger does not hold, needs a look", () => {
    expect(rowProblem(legal("lg-reactive-write-in-owned-scope-new-dead-shape-store-direct-none"))).toBe("unexecuted-violation");
    expect(unexecutedViolation(legal("lg-reactive-write-in-owned-scope-new-dead-shape-store-direct-none"))).toBe(true);
  });

  it("a ledgered one passes, and names its entry", () => {
    const row = legal("lg-reactive-write-in-owned-scope-synthetic-dead-store-direct-none");
    expect(rowProblem(row, { unexecutedKnown: SYNTHETIC })).toBeNull();
    expect(unexecutedEntry(row, SYNTHETIC)).toMatchObject({ kind: "pending-fix", item: "synthetic-dead" });
  });

  it("an entry removed from the ledger (its fix landed) makes the same row fail again", () => {
    const row = legal("lg-reactive-write-in-owned-scope-synthetic-dead-store-direct-none");
    const without = { entries: SYNTHETIC.entries.filter((entry) => entry.item !== "synthetic-dead") };
    expect(rowProblem(row, { unexecutedKnown: SYNTHETIC })).toBeNull();
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
  });
});

describe("the ledger", () => {
  it("every entry names a legal grid, a kind, a pattern that compiles, an ADR of docs/decisions.md and a reason (the ledger may be empty)", () => {
    expect(Array.isArray(UNEXECUTED_KNOWN.entries)).toBe(true);
    for (const entry of [...UNEXECUTED_KNOWN.entries, ...SYNTHETIC.entries]) {
      expect(entry.grid).toMatch(/^lg-/);
      expect(["explained", "pending-fix", "known-false-positive"]).toContain(entry.kind);
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

  it("a known-false-positive entry names an ADR of docs/decisions.md that says why the site is not proved dead", () => {
    for (const entry of UNEXECUTED_KNOWN.entries.filter((e) => e.kind === "known-false-positive")) {
      const number = entry.adr.replace("ADR ", "");
      expect(decisions).toContain(`<a id="adr-${number}"></a>`);
    }
  });

  it("holds the empty NodeListOf of a selector as a known false positive (ADR 1033), and nothing next to it", () => {
    expect(unexecutedEntry(legal("lg-action-called-in-owned-scope-tbl-nodelistof-foreach-zero-x-direct-none"))).toMatchObject({ kind: "known-false-positive", adr: "ADR 1033" });
    // ADR 1033 proves the plain NodeList shape dead, so it is no longer held; nor are another code at a dead bundle or another empty carrier.
    expect(rowProblem(legal("lg-reactive-write-in-owned-scope-tbl-nodelist-foreach-zero-store-direct-none"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-pending-async-forbidden-scope-dead-bundle-spread-x-direct-none"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-action-called-in-owned-scope-tbl-set-foreach-zero-x-direct-none"))).toBe("unexecuted-violation");
  });

  it("a group whose fix landed is out of the ledger, so its shape fails again if it comes back (tbl-zero, ADR 1019; gate-throw, ADR 1010; ob-async-never, ADR 1021; apply-void-value, ADR 1020; dead-bundle SYNC_NODE, ADR 1034)", () => {
    expect(rowProblem(legal("lg-action-called-in-owned-scope-tbl-map-foreach-zero-x-direct-none"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-reactive-write-in-owned-scope-gate-throw-store-destructured-helper"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-untracked-read-after-await-ob-getter-memo-async-never-x-direct-helper"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-sync-node-received-async-apply-void-value-untrack-memo-alias-iife"))).toBe("unexecuted-violation");
    expect(rowProblem(legal("lg-sync-node-received-async-dead-bundle-spread-effect-direct-closure"))).toBe("unexecuted-violation");
  });
});
