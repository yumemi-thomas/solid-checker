// What the review plan says about the bytes schema v1 cannot pin.
//
// `contractArtifacts` hashes the one implementation artifact a v1 contract can
// carry and then answers the question that hash cannot: how many *further*
// modules the analysis read, whose bytes every summary also depends on. That
// count is read off the attested closure record rather than re-walked, so the
// two cannot disagree -- which makes the record's own failure shapes this
// function's problem. A record that names no module is a generation whose
// closure was not derived; reading it as "one module, nothing pulled in" would
// silently suppress the note over exactly the generation that most needs it.

import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { contractArtifacts } from "../scripts/generate-package-contract.mjs";

/// One package with one runtime target, and whatever closure records a test
/// wants to hand the binding. `modules` entries only ever need a `path` here:
/// nothing in this function reads a hash it did not compute itself.
function bind(records, { entrypoints } = {}) {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-binding-"));
  try {
    writeFileSync(join(directory, "index.js"), "export const thing = 1;\n");
    const names = Object.keys(records);
    return contractArtifacts(
      join(directory, "solid-reactivity.json"),
      directory,
      new Map(names.map(name => [name, new Set(["./index.js"])])),
      entrypoints ?? Object.fromEntries(names.map(name => [name, { exports: {} }])),
      {
        entrypoints: Object.fromEntries(
          Object.entries(records).map(([name, modules]) => [
            name,
            { targets: ["./index.js"], modules: modules.map(path => ({ path })) }
          ])
        )
      }
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("the artifact is hashed and a single-module record pulls nothing in", () => {
  const binding = bind({ ".": ["index.js"] });
  assert.equal(binding.artifacts.implementation.path, "index.js");
  assert.match(binding.artifacts.implementation.hash, /^sha256:[0-9a-f]{64}$/);
  assert.deepEqual(binding.notes, []);
});

test("the unpinned remainder is counted off the record", () => {
  const binding = bind({ ".": ["index.js", "impl.js", "deep.js"] });
  assert.equal(binding.notes.length, 1);
  assert.match(binding.notes[0], /\.\/index\.js pulls in 2 further module\(s\)/);
});

test("an entrypoint whose closure was not derived cannot suppress the count", () => {
  // The live shape this guards: `generationClosures` catches a refusal while
  // deriving one entrypoint's closure, records `closure not recorded`, and
  // leaves that entrypoint's `modules` empty. It is still the first record in
  // iteration order, and the predecessor read it -- an empty array is an array
  // -- so `0 - 1` came out negative and the note vanished for every entrypoint
  // that *did* derive a closure.
  const binding = bind({ "./a": [], "./b": ["index.js", "impl.js"] });
  assert.equal(binding.notes.length, 1);
  assert.match(binding.notes[0], /pulls in 1 further module\(s\)/);
});

test("no record at all says nothing extra here", () => {
  // Every entrypoint refused before a closure was derived. The per-entrypoint
  // `notes` already carry why, and inventing a count from nothing would be the
  // opposite failure to the one above.
  const binding = bind({ "./a": [] });
  assert.deepEqual(binding.notes, []);
  assert.match(binding.artifacts.implementation.hash, /^sha256:/);
});

test("two records over one target that disagree are reported, not averaged", () => {
  // One target has no sibling to exclude, so two records over it cannot
  // legitimately differ; if they do, this generation contradicted itself. The
  // smaller count must not be the one a reviewer is handed, and neither may the
  // larger one be passed off as established.
  const binding = bind({ ".": ["index.js", "impl.js"], "./b": ["index.js"] });
  assert.equal(binding.notes.length, 1);
  assert.match(
    binding.notes[0],
    /the closure records for \.\/index\.js name different module counts \(1, 2\)/
  );
  assert.doesNotMatch(binding.notes[0], /pulls in/);
});

test("a record for an entrypoint the contract did not emit is not a target", () => {
  // `targets` is built from the entrypoints that survived emission, so a record
  // belonging to a refused one cannot introduce a second target and turn a
  // byte-bound contract into an unbound one.
  const binding = bind(
    { ".": ["index.js"], "./refused": ["index.js"] },
    { entrypoints: { ".": { exports: {} } } }
  );
  assert.equal(binding.artifacts.implementation.path, "index.js");
  assert.deepEqual(binding.notes, []);
});
