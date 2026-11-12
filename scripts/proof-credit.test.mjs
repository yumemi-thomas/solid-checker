// Batch 11, slice 0: the proof credit fold (scripts/lib/proof-credit.mjs), on hand-written log lines.
import assert from "node:assert/strict";
import { test } from "vitest";

import { SITE_CODE, creditOf, creditSummary, siteCredits } from "./lib/proof-credit.mjs";

const F = "/p/App.tsx";
const site = (t, s, e, extra = {}) => ({ k: "site", f: F, t, s, e, source: "accessor", via: "call", alt: [], end: "none", detail: "", ...extra });
const verdict = (s, e, rule, code, kind) => ({ k: "verdict", f: F, s, e, rule, code, kind });

test("a finding of the rule that claims the code credits it, violation or obligation; an obligation of a declining rule declines it; any other finding is another rule's", () => {
  assert.equal(creditOf("STRICT_READ_UNTRACKED", { rule: "strict-read-untracked", kind: "violation" }), "claims");
  assert.equal(creditOf("STRICT_READ_UNTRACKED", { rule: "strict-read-untracked", kind: "uncertifiable" }), "claims");
  assert.equal(creditOf("STRICT_READ_UNTRACKED", { rule: "reactive-dispatch-unresolved", kind: "uncertifiable" }), null);
  assert.equal(creditOf("CONTEXT_NOT_FOUND", { rule: "missing-owner", kind: "uncertifiable" }), "declines");
  assert.equal(creditOf("CONTEXT_NOT_FOUND", { rule: "missing-owner", kind: "violation" }), null);
  assert.equal(creditOf("CONTEXT_NOT_FOUND", { rule: "missing-context-provider", kind: "uncertifiable" }), "claims");
  assert.equal(creditOf("STORE_WRITE_OUTSIDE_SETTER", { rule: "strict-read-untracked", kind: "violation" }), null);
});

test("each site gets the code of its table, the findings at it or at an alternate span, and who credits it", () => {
  const credits = siteCredits([
    site("read", 10, 15), // a claiming violation
    verdict(10, 15, "strict-read-untracked", "SC1001", "violation"),
    site("read", 20, 25, { alt: [[21, 26]] }), // another rule's obligation at the alternate span
    verdict(21, 26, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    site("write", 30, 35), // a violation of another rule and an obligation: both listed, the violation first
    verdict(30, 35, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    verdict(30, 35, "strict-read-untracked", "SC1001", "violation"),
    site("store-write", 40, 45), // nothing stands there
    site("on-flush", 50, 55),
    verdict(50, 55, "on-flush-in-owned-scope", "SC3002", "violation"),
    site("mystery-table", 60, 65),
    verdict(60, 65, "strict-read-untracked", "SC1001", "violation"),
  ]);
  assert.deepEqual(credits.map((credit) => [credit.table, credit.code, credit.credited]), [
    ["read", "STRICT_READ_UNTRACKED", "claims"],
    ["read", "STRICT_READ_UNTRACKED", "other"],
    ["write", "REACTIVE_WRITE_IN_OWNED_SCOPE", "other"],
    ["store-write", "STORE_WRITE_OUTSIDE_SETTER", "none"],
    ["on-flush", "ON_FLUSH_IN_OWNED_SCOPE", "claims"],
    ["mystery-table", null, "other"],
  ]);
  assert.deepEqual(credits[2].by.map((one) => `${one.kind}:${one.rule}`), ["violation:strict-read-untracked", "uncertifiable:reactive-dispatch-unresolved"]);
  assert.deepEqual(Object.keys(SITE_CODE).sort(), ["action", "on-flush", "read", "store-write", "write"]);
});

test("a finding with no rule (a log from before the field) is credited to nobody, never guessed", () => {
  const [credit] = siteCredits([site("read", 1, 2), { k: "verdict", f: F, s: 1, e: 2, code: "SC1001", kind: "violation" }]);
  assert.equal(credit.credited, "other");
  assert.deepEqual(credit.by, [{ rule: null, id: "SC1001", kind: "violation" }]);
});

test("the summary counts per code and names the rules that end sites only as another rule", () => {
  const credits = siteCredits([
    site("read", 1, 2), verdict(1, 2, "strict-read-untracked", "SC1001", "violation"),
    site("read", 3, 4), verdict(3, 4, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    site("read", 5, 6), verdict(5, 6, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    site("write", 7, 8),
  ]);
  const summary = creditSummary(credits);
  assert.equal(summary.sites, 4);
  assert.deepEqual(summary.byCredit, { claims: 1, declines: 0, other: 2, none: 1 });
  assert.deepEqual(summary.perCode.STRICT_READ_UNTRACKED, { sites: 3, claims: 1, declines: 0, other: 2, none: 0 });
  assert.deepEqual(summary.otherBy, { "STRICT_READ_UNTRACKED <- reactive-dispatch-unresolved": 2 });
});
