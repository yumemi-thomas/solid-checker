// Batch 11, slice 0: the candidate fold (scripts/lib/candidates.mjs), on hand-written log lines.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { foldDirectory } from "./proof-candidates.mjs";
import { OWNER_CONTEXT_LEAF, OWNER_CONTEXT_LEAF_OWNED, addCandidate, candidatesOf, factsOf, isTestFile, mergeTally, newTally, tallyCandidates } from "./lib/candidates.mjs";

const F = "/p/App.tsx";
const candidate = (rule, t, s, e, extra = {}) => ({ k: "candidate", f: F, g: "demo/p", rule, t, s, e, alt: [], facts: {}, ...extra });
const verdict = (s, e, rule, code, kind, f = F) => ({ k: "verdict", f, s, e, rule, code, kind });
const ends = (lines) => candidatesOf(lines).map((one) => `${one.rule}@${one.s}:${one.end}`);

test("a candidate ends in a finding of its rule, another rule's finding only, a named silence, the compiler's discard, or nothing", () => {
  const lines = [
    candidate("SC2004", "resolve", 10, 17, { alt: [[10, 30]] }), // the rule's own violation at the callee
    verdict(10, 17, "no-await-primitive-in-tracked-scope", "SC2004", "violation"),
    candidate("SC2004", "resolve", 40, 47, { alt: [[40, 60]] }), // its own obligation, at the callee
    verdict(40, 47, "no-await-primitive-in-tracked-scope", "SC2004", "uncertifiable"),
    candidate("SC2005", "until", 70, 75, { alt: [[70, 90]] }), // only another rule's obligation, at the whole call
    verdict(70, 90, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    candidate("SC6002", "create-in-effect", 100, 110, { alt: [[100, 120]] }), // a named silence
    { k: "silent", f: F, s: 100, e: 110, rule: "end", proof: "local_access.discover_call_as.handed-silent" },
    candidate("SC7001", "effect-function-slot", 130, 140, { facts: { discarded: "yes" } }),
    candidate("SC7002", "sync-option", 150, 160, { facts: { sync: "absent", discarded: "no" } }),
  ];
  assert.deepEqual(ends(lines), ["SC2004@10:finding-violation", "SC2004@40:finding-obligation", "SC2005@70:other", "SC6002@100:silent", "SC7001@130:discarded", "SC7002@150:unaccounted"]);
  // a finding wins over a silence and a discard; another rule's finding over a silence
  const both = [candidate("SC2006", "flush-in-action", 1, 2, { facts: { discarded: "yes" } }), verdict(1, 2, "flush-in-action", "SC2006", "violation"), { k: "silent", f: F, s: 1, e: 2, rule: "end", proof: "x.y.z" }];
  assert.deepEqual(ends(both), ["SC2006@1:finding-violation"]);
  const rival = [candidate("SC2006", "flush-in-action", 1, 2), verdict(1, 2, "strict-read-untracked", "SC1001", "violation"), { k: "silent", f: F, s: 1, e: 2, rule: "end", proof: "x.y.z" }];
  const [found] = candidatesOf(rival);
  assert.equal(found.end, "other");
  assert.deepEqual(found.otherRules, ["strict-read-untracked"]);
});

test("a finding is the rule's by its id, in the file of the candidate and not another's", () => {
  const lines = [candidate("SC7002", "sync-option", 5, 10), verdict(5, 10, "sync-computation-received-async", "SC7002", "violation", "/p/Other.tsx")];
  assert.deepEqual(ends(lines), ["SC7002@5:unaccounted"]);
});

test("the flush census is answered by either flush rule's finding inside the call", () => {
  const lines = [
    candidate("flush", "census", 100, 150, { facts: { called: "yes", caller: "no", drainsNothing: "no" } }),
    verdict(120, 127, "flush-in-effect-callback", "SC2008", "uncertifiable"), // at the callee, inside the call
    candidate("flush", "census", 200, 210, { facts: { called: "no", caller: "no", drainsNothing: "no" } }),
    candidate("flush", "census", 300, 340, { facts: { called: "yes", caller: "yes", drainsNothing: "no" } }),
    verdict(310, 315, "strict-read-untracked", "SC1001", "violation"),
  ];
  assert.deepEqual(ends(lines), ["flush@100:finding-obligation", "flush@200:unaccounted", "flush@300:other"]);
});

test("SC3001's candidates are the owner sites that may run under a leaf owner; its own finding inside the site answers", () => {
  const lines = [
    { k: "owner", f: F, s: 10, e: 40, rule: "SC4001", ctx: OWNER_CONTEXT_LEAF },
    verdict(20, 30, "leaf-owner-forbidden-call", "SC3001", "violation"),
    { k: "owner", f: F, s: 50, e: 80, rule: "SC4001", ctx: OWNER_CONTEXT_LEAF_OWNED | 1 },
    { k: "owner", f: F, s: 90, e: 99, rule: "SC4001", ctx: 1 }, // owned only: no leaf, no candidate
    { k: "owner", f: F, s: 100, e: 110, rule: "SC4001", ctx: 2 },
  ];
  const found = candidatesOf(lines);
  assert.deepEqual(found.map((one) => `${one.rule}@${one.s}:${one.end}`), ["SC3001@10:finding-violation", "SC3001@50:unaccounted"]);
  assert.deepEqual(found.map((one) => one.facts), [{ leaf: "yes", leafOwned: "no" }, { leaf: "no", leafOwned: "yes" }]);
});

test("SC2009's candidates are the optimistic setter calls: the write universe's silences for them, and the sites of its own findings", () => {
  const lines = [
    { k: "silent", f: F, s: 10, e: 20, rule: "end", proof: "local_access.push_setter_write.optimistic-signal-unguarded" },
    { k: "silent", f: F, s: 30, e: 40, rule: "end", proof: "local_access.push_setter_write.optimistic-store-unguarded" },
    verdict(30, 40, "optimistic-write-before-load", "SC2009", "violation"),
    { k: "silent", f: F, s: 50, e: 60, rule: "end", proof: "local_access.discover_member.props-static" }, // another silence: not an optimistic write
    verdict(70, 80, "optimistic-write-before-load", "SC2009", "uncertifiable"),
  ];
  // the silence at a candidate's own span is the SC2001 write site's, not an answer to SC2009
  assert.deepEqual(ends(lines).sort(), ["SC2009@10:unaccounted", "SC2009@30:finding-violation", "SC2009@70:finding-obligation"]);
});

test("a tally counts each ending, splits by each fact (not the discard), keeps who ended it as another rule, and counts test files apart", () => {
  const lines = [
    candidate("SC7002", "sync-option", 1, 2, { facts: { sync: "true", discarded: "no" } }),
    verdict(1, 2, "sync-computation-received-async", "SC7002", "violation"),
    candidate("SC7002", "sync-option", 3, 4, { facts: { sync: "absent", discarded: "no" } }),
    candidate("SC7002", "sync-option", 5, 6, { facts: { sync: "absent", discarded: "no" } }),
    verdict(5, 6, "reactive-dispatch-unresolved", "SC9012", "uncertifiable"),
    candidate("SC7002", "sync-option", 7, 8, { facts: { sync: "absent" }, f: "/p/App.test.tsx" }),
  ];
  const tallies = tallyCandidates(candidatesOf(lines));
  const tally = tallies["SC7002 sync-option"];
  assert.equal(tally.candidates, 4);
  assert.deepEqual(tally.ends, { "finding-violation": 1, "finding-obligation": 0, other: 1, silent: 0, discarded: 0, unaccounted: 2 });
  assert.deepEqual(tally.otherBy, { "reactive-dispatch-unresolved": 1 });
  assert.deepEqual(Object.keys(tally.byFact).sort(), ["sync=absent", "sync=true"]);
  assert.equal(tally.byFact["sync=absent"].candidates, 3);
  assert.equal(tally.byFact["sync=absent"].ends.unaccounted, 2);
  assert.deepEqual(tally.inTestFiles, { candidates: 1, ends: { "finding-violation": 0, "finding-obligation": 0, other: 0, silent: 0, discarded: 0, unaccounted: 1 } });
  // two projects merge into a total
  const total = mergeTally(newTally(), tally);
  mergeTally(total, tally);
  assert.equal(total.candidates, 8);
  assert.equal(total.byFact["sync=absent"].ends.unaccounted, 4);
  assert.equal(total.otherBy["reactive-dispatch-unresolved"], 2);
  assert.equal(total.inTestFiles.candidates, 2);
  const one = newTally();
  addCandidate(one, { file: F, facts: {}, end: "silent", otherRules: [] });
  assert.equal(one.ends.silent, 1);
});

test("a test file is named *.test.* or *.spec.*, or sits under __tests__ or __mocks__", () => {
  for (const path of ["/p/a.test.ts", "/p/a.spec.tsx", "/p/__tests__/a.ts", "/p/src/__mocks__/x.js", "/p/a.test.mjs"]) assert.equal(isTestFile(path), true, path);
  for (const path of ["/p/a.ts", "/p/latest.tsx", "/p/test/a.ts", "/p/a.stories.tsx", "/p/a.testing.ts"]) assert.equal(isTestFile(path), false, path);
});

test("an SC1002 read's shape joins whether its async function is a computation's and whether the read is directly in it", () => {
  const lines = [
    candidate("SC1002", "read-after-await", 10, 20, { facts: { via: "call", computation: "yes", nested: "no" } }),
    verdict(10, 20, "reactive-read-after-await", "SC1002", "uncertifiable"),
    candidate("SC1002", "read-after-await", 30, 40, { facts: { via: "call", computation: "yes", nested: "yes" } }),
    candidate("SC1002", "read-after-await", 50, 60, { facts: { via: "member", computation: "no", nested: "no" } }),
  ];
  const found = candidatesOf(lines);
  assert.deepEqual(found.map((one) => factsOf(one).shape), ["computation-direct", "computation-nested", "other-async-direct"]);
  assert.deepEqual(found.map((one) => one.end), ["finding-obligation", "unaccounted", "unaccounted"]);
  const tally = tallyCandidates(found)["SC1002 read-after-await"];
  assert.deepEqual(Object.keys(tally.byFact).filter((key) => key.startsWith("shape=")).sort(), ["shape=computation-direct", "shape=computation-nested", "shape=other-async-direct"]);
  assert.equal(tally.byFact["shape=computation-direct"].ends["finding-obligation"], 1);
  // another rule's facts get no shape
  assert.equal(factsOf({ rule: "SC7002", facts: { sync: "true" } }).shape, undefined);
});

test("the fold counts the logs that hold a candidate line, so a run whose analyses stopped early shows", () => {
  const dir = mkdtempSync(join(tmpdir(), "proof-candidates-"));
  const write = (name, lines) => writeFileSync(join(dir, name), `${lines.map((line) => JSON.stringify(line)).join("\n")}\n`);
  try {
    write("proof-1.jsonl", [candidate("SC7002", "sync-option", 5, 10, { facts: { sync: "absent" } })]);
    // a log with a finding but no candidate line (the analysis ended before the accounting, or the dry run was off), and one with only a silence the fold reads as SC2009's candidate
    write("proof-2.jsonl", [verdict(5, 10, "sync-computation-received-async", "SC7002", "violation")]);
    write("proof-3.jsonl", [{ k: "silent", f: F, s: 30, e: 40, rule: "end", proof: "local_access.push_setter_write.optimistic-signal-unguarded" }]);
    const { logs, total } = foldDirectory(dir);
    assert.deepEqual(logs, { files: 3, withCandidateLines: 1 });
    assert.equal(total["SC7002 sync-option"].candidates, 1);
    assert.equal(total["SC2009 optimistic-write"].candidates, 1); // a candidate, but not from a `candidate` line: it does not count as the dry run's
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
