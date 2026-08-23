import assert from "node:assert/strict";
import test from "node:test";

import {
  attachMergeDivergences,
  narrowedAttributionNotes,
  unknownClaimAttributions
} from "../scripts/generate-package-contract.mjs";

const prefix = "solid-checker:unknown-claim-attribution=";

function marker(note) {
  return `${prefix}${JSON.stringify(note)}`;
}

const marked = {
  obligation: "ReactiveDispatchUnresolved",
  analysisContext: "",
  path: "panel.js",
  startByte: 102,
  endByte: 114,
  mechanism: "reachability",
  domains: ["reactiveReads"],
  exports: ["Reaches"]
};

const narrowed = {
  obligation: "ReactiveDispatchUnresolved",
  analysisContext: "exported-parameter-member-dispatch",
  path: "channel.js",
  startByte: 7,
  endByte: 66,
  mechanism: "reachability",
  domains: ["reactiveReads", "returns"],
  exports: []
};

test("a note naming no export is kept, so the narrowing can be reported", () => {
  const notes = unknownClaimAttributions(
    [marker(marked), marker(narrowed), "unrelated stderr line"].join("\n")
  );
  assert.equal(notes.length, 2);
  assert.deepEqual(notes[1].exports, []);
});

test("a note without an exports array is still a half-note and dropped", () => {
  const { exports: _dropped, ...halfNote } = narrowed;
  assert.deepEqual(unknownClaimAttributions(marker(halfNote)), []);
});

test("a malformed marker line loses its explanation, never the contract", () => {
  assert.deepEqual(unknownClaimAttributions(`${prefix}{not json`), []);
});

test("only the zero-export notes become review-plan notes", () => {
  const notes = narrowedAttributionNotes([
    { ...marked, entrypoint: "." },
    { ...narrowed, entrypoint: "./client" }
  ]);
  assert.equal(notes.length, 1);
  const [note] = notes;
  // Naming the obligation, its location and the rung is the whole point: the
  // reviewer has to be able to go and check the narrowing against the source.
  assert.match(note, /^\.\/client: /);
  assert.match(note, /ReactiveDispatchUnresolved obligation at channel\.js:7-66/);
  assert.match(note, /exported-parameter-member-dispatch/);
  assert.match(note, /`reachability`/);
  assert.match(note, /no claim was marked unknown/);
});

test("a marked note produces no narrowing note", () => {
  assert.deepEqual(narrowedAttributionNotes([marked]), []);
});

// --------------------------------------------------- merge-produced sentinels

test("a merge-produced sentinel carries which branches disagreed and how", () => {
  // `mergeSummaries` is the second emitter of the unknown sentinel and used to
  // be the silent one: a reviewer read "returns is unknown" with nothing saying
  // that one conditional branch proved an accessor and the other proved none.
  const items = [
    {
      id: "unknown-sentinel-1",
      kind: "unknown-sentinel",
      target: { entrypoint: ".", export: "Show", field: "returns" },
      text: ".:Show: returns"
    },
    {
      id: "generated-summary-1",
      kind: "generated-summary",
      target: { entrypoint: ".", export: "Show" },
      text: ".:Show is certified as a function"
    }
  ];
  const [sentinel, summary] = attachMergeDivergences(items, [
    {
      entrypoint: ".",
      export: "Show",
      domain: "returns",
      shape: "one branch proves it and another proves none",
      branches: ["node", "browser"],
      mechanism: "conditional-branch-merge"
    }
  ]);
  assert.deepEqual(sentinel.because.divergences[0].branches, ["browser", "node"]);
  assert.equal(sentinel.because.divergences[0].mechanism, "conditional-branch-merge");
  assert.match(sentinel.because.divergences[0].detail, /the returns claim diverges/);
  assert.match(sentinel.because.divergences[0].detail, /proves it and another proves none/);
  assert.match(sentinel.because.divergences[0].detail, /per-branch claims\s+are in variants/);
  // Only the sentinel item is explained; nothing else gains a reason.
  assert.equal(summary.because, undefined);
});

test("a kind divergence produces no because, because it produces no sentinel", () => {
  // Diverging kinds merge to the conservative `value` surface, which is a
  // claim rather than a sentinel, so there is no `unknown-sentinel` item to
  // explain and attaching one would name a question nobody asked.
  const items = [
    {
      id: "unknown-sentinel-1",
      kind: "unknown-sentinel",
      target: { entrypoint: ".", export: "Show", field: "kind" },
      text: ".:Show: kind"
    }
  ];
  assert.equal(
    attachMergeDivergences(items, [
      {
        entrypoint: ".",
        export: "Show",
        domain: "kind",
        shape: "the branches prove different export kinds",
        branches: ["browser", "node"],
        mechanism: "conditional-branch-merge"
      }
    ])[0].because,
    undefined
  );
});

test("an existing generation attribution survives a merge divergence beside it", () => {
  const items = [
    {
      id: "unknown-sentinel-1",
      kind: "unknown-sentinel",
      target: { entrypoint: ".", export: "Show", field: "returns" },
      text: ".:Show: returns",
      because: { attributions: [{ obligation: "UnresolvedDispatch", mechanism: "reachability" }] }
    }
  ];
  const [item] = attachMergeDivergences(items, [
    {
      entrypoint: ".",
      export: "Show",
      domain: "returns",
      shape: "the branches prove different values",
      branches: ["browser", "node"],
      mechanism: "conditional-branch-merge"
    }
  ]);
  assert.equal(item.because.attributions.length, 1);
  assert.equal(item.because.divergences.length, 1);
});
