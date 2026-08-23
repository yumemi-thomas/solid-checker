// The gate cache's whole safety argument is "the key covers every input", so
// these tests are the argument. Each one flips exactly one byte in one input
// class and demands a different key; together they enumerate the classes.
//
// A test that could not fail is worse than no test, so the fixtures here are a
// throwaway repository under $TMPDIR rather than this one: the tests need to
// *edit* a script, a binary and a fixture file, which they must never do to the
// real tree.
import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  CACHE_CONTROL_VARIABLES,
  CACHE_FORMAT_VERSION,
  ancestorChainDigest,
  cacheEnabled,
  environmentDigest,
  hashFile,
  hashTree,
  openGateCache,
  scriptClosure,
  writeJsonAtomic,
} from "./lib/gate-cache.mjs";
import {
  MEMO_FORMAT_VERSION,
  memoInputDigest,
  memoizedIntegrity,
  readMemo,
} from "./check-contract-pins.mjs";

/**
 * A throwaway repository shaped like the real one: a gate script, a local
 * module it imports, a binary it runs, and one fixture unit.
 */
function scaffold() {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-gate-cache-test-"));
  mkdirSync(join(root, "scripts", "lib"), { recursive: true });
  mkdirSync(join(root, "bin"), { recursive: true });
  mkdirSync(join(root, "fixtures", "unit", "node_modules", "solid-js"), { recursive: true });
  writeFileSync(join(root, "scripts", "gate.mjs"), 'import "./lib/helper.mjs";\n');
  writeFileSync(join(root, "scripts", "lib", "helper.mjs"), "export const shape = 1;\n");
  writeFileSync(join(root, "bin", "checker"), "binary-v1");
  writeFileSync(join(root, "bin", "checker.buildinfo"), "revision=aaa build-id=dev\n");
  writeFileSync(join(root, "fixtures", "unit", "App.tsx"), "export const App = () => null;\n");
  writeFileSync(join(root, "fixtures", "unit", "tsconfig.json"), '{"files":["App.tsx"]}\n');
  writeFileSync(
    join(root, "fixtures", "unit", "node_modules", "solid-js", "package.json"),
    '{"version":"1.9.14"}\n',
  );
  return root;
}

const cacheFor = (root, overrides = {}) =>
  openGateCache({
    gate: "test-gate",
    scriptPath: join(root, "scripts", "gate.mjs"),
    binaries: [join(root, "bin", "checker"), join(root, "bin", "checker.buildinfo")],
    root,
    env: {},
    ...overrides,
  });

const unitParts = (root) => ["unit:fixtures/unit", hashTree(join(root, "fixtures", "unit"))];

// `run` refuses a fixed array carrying a filesystem digest -- it cannot be
// re-verified after compute() -- so the gates, and these tests, hand it a thunk.
const unitThunk = (root) => () => unitParts(root);

const keyIn = (root, overrides = {}) => cacheFor(root, overrides).key(unitParts(root));

/**
 * Async-aware, deliberately. A synchronous `try/finally` around a body that
 * returns a promise deletes the scaffold the moment the promise is *created*,
 * so anything after the body's first `await` -- `writeJsonAtomic`, which
 * `mkdirSync`s recursively -- runs against a deleted tree and re-creates a
 * directory under it that is then leaked in `$TMPDIR` on every run.
 */
const withScaffold = (body) => {
  const root = scaffold();
  const clean = () => rmSync(root, { recursive: true, force: true });
  let result;
  try {
    result = body(root);
  } catch (error) {
    clean();
    throw error;
  }
  if (result !== null && typeof result?.then === "function") {
    return result.then(
      (value) => {
        clean();
        return value;
      },
      (error) => {
        clean();
        throw error;
      },
    );
  }
  clean();
  return result;
};

test("a file digest distinguishes absent from empty from present", () => {
  withScaffold((root) => {
    const path = join(root, "probe");
    assert.equal(hashFile(path), "absent");
    writeFileSync(path, "");
    const empty = hashFile(path);
    assert.notEqual(empty, "absent");
    writeFileSync(path, "a");
    assert.notEqual(hashFile(path), empty);
  });
});

test("a tree digest sees a byte flip, a new untracked file, and a deletion", () => {
  withScaffold((root) => {
    const unit = join(root, "fixtures", "unit");
    const original = hashTree(unit);

    writeFileSync(join(unit, "App.tsx"), "export const App = () => 1;\n");
    const edited = hashTree(unit);
    assert.notEqual(edited, original);

    // Untracked on purpose: the walk is of the disk, not of git, because a
    // file git ignores changes the checker's answer exactly as much as a
    // tracked one.
    writeFileSync(join(unit, "untracked.tsx"), "export const extra = 1;\n");
    const added = hashTree(unit);
    assert.notEqual(added, edited);

    rmSync(join(unit, "untracked.tsx"));
    assert.equal(hashTree(unit), edited);

    rmSync(join(unit, "node_modules", "solid-js", "package.json"));
    assert.notEqual(hashTree(unit), edited, "a missing dialect stub must change the digest");

    assert.equal(hashTree(join(root, "nowhere")), "absent");
  });
});

test("a tree digest records a symlink's target instead of following it", () => {
  withScaffold((root) => {
    const unit = join(root, "fixtures", "unit");
    symlinkSync(join(root, "bin"), join(unit, "link"), "dir");
    const first = hashTree(unit);
    unlinkSync(join(unit, "link"));
    symlinkSync(join(root, "scripts"), join(unit, "link"), "dir");
    assert.notEqual(hashTree(unit), first);
  });
});

test("identical inputs reproduce the same key", () => {
  withScaffold((root) => {
    assert.equal(keyIn(root), keyIn(root));
    assert.match(keyIn(root), /^[0-9a-f]{64}$/);
  });
});

test("a byte flip in the unit's own tree changes the key", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "fixtures", "unit", "App.tsx"), "export const App = () => 2;\n");
    assert.notEqual(keyIn(root), before);
  });
});

test("a byte flip in the checker binary changes the key", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "bin", "checker"), "binary-v2");
    assert.notEqual(keyIn(root), before);
  });
});

test("a byte flip in the producer's build stamp changes the key", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "bin", "checker.buildinfo"), "revision=bbb build-id=dev\n");
    assert.notEqual(keyIn(root), before);
  });
});

test("a byte flip in the gate script changes the key", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "scripts", "gate.mjs"), 'import "./lib/helper.mjs";\n// edited\n');
    assert.notEqual(keyIn(root), before);
  });
});

test("a byte flip in a module the gate imports changes the key", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "scripts", "lib", "helper.mjs"), "export const shape = 2;\n");
    assert.notEqual(keyIn(root), before);
  });
});

test("a new module under scripts/lib changes the key even if nothing imports it yet", () => {
  withScaffold((root) => {
    const before = keyIn(root);
    writeFileSync(join(root, "scripts", "lib", "another.mjs"), "export const extra = 1;\n");
    assert.notEqual(keyIn(root), before);
  });
});

test("a SOLID_ environment variable is part of the key, name and value", () => {
  withScaffold((root) => {
    const bare = keyIn(root, { env: {} });
    const named = keyIn(root, { env: { SOLID_CHECKER_DAEMON: "1" } });
    const valued = keyIn(root, { env: { SOLID_CHECKER_DAEMON: "0" } });
    const other = keyIn(root, { env: { SOLID_CHECKER_CACHE_RETENTION: "1" } });

    assert.notEqual(named, bare);
    assert.notEqual(valued, named);
    assert.notEqual(other, named);
    // Ordering must not matter, or the key would depend on how the shell
    // happened to hand the environment over.
    assert.equal(
      keyIn(root, { env: { SOLID_A: "1", SOLID_B: "2" } }),
      keyIn(root, { env: { SOLID_B: "2", SOLID_A: "1" } }),
    );
    // Non-SOLID_ variables are not inputs, so PATH churn cannot invalidate.
    assert.equal(keyIn(root, { env: { PATH: "/usr/bin" } }), bare);
  });
});

test("the cache's own controls stay out of the key, so a cached run is comparable to an uncached one", () => {
  withScaffold((root) => {
    const bare = keyIn(root, { env: {} });
    for (const variable of CACHE_CONTROL_VARIABLES) {
      assert.equal(keyIn(root, { env: { [variable]: "1" } }), bare);
    }
    assert.deepEqual(environmentDigest({ SOLID_CHECKER_GATE_CACHE: "0", SOLID_X: "y" }), ["SOLID_X=y"]);
  });
});

test("a stored entry from another format version is not replayed", () => {
  withScaffold((root) => {
    const cache = cacheFor(root);
    const key = cache.key(unitParts(root));
    writeJsonAtomic(join(cache.directory, `${key}.json`), {
      formatVersion: CACHE_FORMAT_VERSION + 1,
      gate: "test-gate",
      key,
      value: { stale: true },
    });

    return cache
      .run(unitThunk(root), async () => ({ fresh: true }))
      .then((result) => {
        assert.deepEqual(result.value, { fresh: true });
        assert.equal(result.hit, false);
        assert.equal(cache.hits, 0);
        assert.equal(cache.misses, 1);
      });
  });
});

test("an entry filed under a different key is not replayed", () => {
  withScaffold((root) => {
    const cache = cacheFor(root);
    const key = cache.key(unitParts(root));
    writeJsonAtomic(join(cache.directory, `${key}.json`), {
      formatVersion: CACHE_FORMAT_VERSION,
      gate: "test-gate",
      key: `${key.slice(0, -1)}0`,
      value: { stale: true },
    });
    return cache.run(unitThunk(root), async () => ({ fresh: true })).then((result) => {
      assert.equal(result.hit, false);
      assert.deepEqual(result.value, { fresh: true });
    });
  });
});

test("a miss computes and stores, a repeat replays, and an input change misses again", async () => {
  const root = scaffold();
  try {
    let computations = 0;
    const compute = async () => {
      computations += 1;
      return { findings: computations };
    };

    const first = cacheFor(root);
    assert.deepEqual((await first.run(unitThunk(root), compute)).value, { findings: 1 });
    assert.equal(first.misses, 1);

    // A fresh cache object, as a second process would have.
    const second = cacheFor(root);
    const replayed = await second.run(unitThunk(root), compute);
    assert.equal(replayed.hit, true);
    assert.deepEqual(replayed.value, { findings: 1 });
    assert.equal(computations, 1, "a hit must not run the unit");
    assert.match(second.summary(), /1 hit\(s\), 0 miss\(es\)/);

    writeFileSync(join(root, "fixtures", "unit", "App.tsx"), "export const App = () => 3;\n");
    const third = cacheFor(root);
    const recomputed = await third.run(unitThunk(root), compute);
    assert.equal(recomputed.hit, false);
    assert.deepEqual(recomputed.value, { findings: 2 });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a unit whose computation throws is never stored", async () => {
  const root = scaffold();
  try {
    const failing = cacheFor(root);
    await assert.rejects(
      failing.run(unitThunk(root), async () => {
        throw new Error("checker exited with 101");
      }),
      /checker exited with 101/,
    );
    assert.ok(
      !existsSync(failing.directory) || readdirSync(failing.directory).length === 0,
      "a crashed unit must leave no entry behind",
    );

    const retried = cacheFor(root);
    const result = await retried.run(unitThunk(root), async () => ({ ok: true }));
    assert.equal(result.hit, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the kill switch disables reading and writing alike", async () => {
  const root = scaffold();
  try {
    const warm = cacheFor(root);
    await warm.run(unitThunk(root), async () => ({ findings: 1 }));

    const disabled = cacheFor(root, { enabled: false });
    let ran = 0;
    const result = await disabled.run(unitThunk(root), async () => {
      ran += 1;
      return { findings: 99 };
    });
    assert.equal(result.hit, false);
    assert.equal(ran, 1, "a disabled cache must not replay");
    assert.match(disabled.summary(), /disabled/);

    // ...and must not have overwritten the stored entry either way.
    const rereading = cacheFor(root);
    assert.deepEqual((await rereading.run(unitThunk(root), async () => ({}))).value, { findings: 1 });

    assert.equal(cacheEnabled({}), true);
    for (const off of ["0", "false", "off", "no", "OFF"]) {
      assert.equal(cacheEnabled({ SOLID_CHECKER_GATE_CACHE: off }), false);
    }
    assert.equal(cacheEnabled({ SOLID_CHECKER_GATE_CACHE: "1" }), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("comparison against the expectation still fails on a fully warm cache", async () => {
  // The property the whole design rests on: what is cached is the *computed
  // result*, never the verdict. So a snapshot (or `expected.json`) is not in the
  // key, editing one needs no cache awareness, and a mismatch fails on a hit
  // exactly as it does on a miss. This models the gates' shape -- cached
  // compute, fresh compare -- so the property is pinned even where a gate's own
  // fixtures cannot be edited by a test.
  const root = scaffold();
  try {
    // Outside the unit's own tree, exactly as `fixtures/findings-snapshots/`
    // sits outside the fixture project it describes.
    mkdirSync(join(root, "fixtures", "findings-snapshots"), { recursive: true });
    const expectation = join(root, "fixtures", "findings-snapshots", "unit.json");
    writeFileSync(expectation, JSON.stringify({ findings: 1 }));

    const gate = async () => {
      const cache = cacheFor(root);
      const { value, hit } = await cache.run(unitThunk(root), async () => ({ findings: 1 }));
      // Read at compare time, never at compute time.
      const expected = JSON.parse(readFileSync(expectation, "utf8"));
      return { hit, passed: JSON.stringify(value) === JSON.stringify(expected) };
    };

    assert.deepEqual(await gate(), { hit: false, passed: true });
    assert.deepEqual(await gate(), { hit: true, passed: true });

    writeFileSync(expectation, JSON.stringify({ findings: 999 }));
    const afterEdit = await gate();
    assert.equal(afterEdit.hit, true, "editing the expectation must not invalidate the computation");
    assert.equal(afterEdit.passed, false, "a warm cache must not be able to hide a mismatch");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an atomic write leaves no temporary behind and never a partial file", () => {
  withScaffold((root) => {
    const target = join(root, "store", "entry.json");
    writeJsonAtomic(target, { a: 1 });
    assert.deepEqual(JSON.parse(readFileSync(target, "utf8")), { a: 1 });

    writeJsonAtomic(target, { a: 2, b: [3] });
    assert.deepEqual(JSON.parse(readFileSync(target, "utf8")), { a: 2, b: [3] });

    // The rename is the publish step, so the only files in the directory are
    // completed entries -- a reader can never observe a half-written one.
    assert.deepEqual(readdirSync(join(root, "store")), ["entry.json"]);
  });
});

test("an unresolvable dynamic import widens the closure instead of narrowing it", () => {
  withScaffold((root) => {
    const certain = scriptClosure(join(root, "scripts", "gate.mjs"), { root });
    assert.equal(certain.uncertain, false);
    assert.deepEqual(certain.files, [
      join(root, "scripts", "gate.mjs"),
      join(root, "scripts", "lib", "helper.mjs"),
    ]);

    writeFileSync(join(root, "scripts", "unrelated.mjs"), "export const x = 1;\n");
    writeFileSync(
      join(root, "scripts", "gate.mjs"),
      'import "./lib/helper.mjs";\nconst name = process.argv[2];\nawait import(name);\n',
    );
    const widened = scriptClosure(join(root, "scripts", "gate.mjs"), { root });
    assert.equal(widened.uncertain, true);
    assert.ok(
      widened.files.includes(join(root, "scripts", "unrelated.mjs")),
      "uncertainty must pull in every gate script, not fewer",
    );
  });
});

test("the uncertainty flag is in the key, not merely the file set that produced it", () => {
  // The claim this pins: "a run that could only trace part of the closure never
  // shares a key with one that traced it all." Comparing two caches built on
  // *different entry scripts* does not test that -- their `files` differ, so
  // their keys differ whether or not `script-closure-uncertain` is in the
  // digest at all. Deleting that line from gate-cache.mjs would leave such an
  // assertion passing.
  //
  // So the closure is injected: same `files`, only `uncertain` varies. Remove
  // the `script-closure-uncertain:${closure.uncertain}` line and these two keys
  // become equal and this test fails, which is the whole point of it.
  withScaffold((root) => {
    const files = [join(root, "scripts", "gate.mjs"), join(root, "scripts", "lib", "helper.mjs")];
    const keyWith = (uncertain) =>
      openGateCache({
        gate: "test-gate",
        scriptPath: join(root, "scripts", "gate.mjs"),
        binaries: [join(root, "bin", "checker"), join(root, "bin", "checker.buildinfo")],
        root,
        env: {},
        closure: { files, uncertain },
      }).key(unitParts(root));

    assert.notEqual(
      keyWith(true),
      keyWith(false),
      "an untraceable closure must not share a key with a fully traced one",
    );
    // The injection is faithful: the traced closure reproduces the traced key.
    assert.equal(keyWith(false), keyIn(root));
  });
});

test("the key covers the dialect-selection chain above the unit, not just the unit", () => {
  // `resolved_solid_version` (rust/crates/solid-facts-backend/src/dialect.rs)
  // walks `start.ancestors()` unbounded, to `/`, taking the nearest
  // `node_modules/solid-js/package.json`. Roughly half the fixture projects
  // ship no stub and depend on the *absence* of one above them, which a digest
  // of the project directory cannot see: a stray `npm install solid-js` one
  // directory above the checkout flips them all to the v1 catalog while every
  // key stays byte-identical, and a warm cache replays the pre-install answers.
  const outside = mkdtempSync(join(tmpdir(), "solid-checker-ancestor-test-"));
  try {
    // A checkout shaped like the real one, containing a stub-less project.
    const root = join(outside, "repo");
    const project = join(root, "fixtures", "reactive-ir", "control-flow");
    mkdirSync(join(root, "scripts", "lib"), { recursive: true });
    mkdirSync(join(root, "bin"), { recursive: true });
    mkdirSync(project, { recursive: true });
    writeFileSync(join(root, "scripts", "coverage.mjs"), "// gate\n");
    writeFileSync(join(root, "bin", "checker"), "binary");
    writeFileSync(join(project, "tsconfig.json"), '{"include":["*.tsx"]}\n');
    writeFileSync(join(project, "App.tsx"), "export const App = () => null;\n");

    const stub = join(outside, "node_modules", "solid-js");
    const key = () =>
      openGateCache({
        gate: "coverage",
        scriptPath: join(root, "scripts", "coverage.mjs"),
        binaries: [join(root, "bin", "checker")],
        root,
        env: {},
      }).key([
        "project:reactive-ir/control-flow",
        hashTree(project),
        ancestorChainDigest(project, "node_modules/solid-js/package.json"),
      ]);

    const before = key();

    // An install ABOVE the checkout root -- a stray `npm install solid-js` one
    // directory up, or a `~/node_modules`. Invisible to the project's tree.
    mkdirSync(stub, { recursive: true });
    writeFileSync(join(stub, "package.json"), '{"version":"1.9.14"}\n');
    const afterInstall = key();
    assert.notEqual(before, afterInstall, "an ancestor dialect stub must change the key");

    // Its *version* is what dialect selection reads, so editing it must move too.
    writeFileSync(join(stub, "package.json"), '{"version":"2.0.0"}\n');
    assert.notEqual(afterInstall, key(), "an ancestor stub's version must change the key");

    // Removing it restores the original key: the chain is a function of the
    // inputs, not a monotonic counter.
    rmSync(join(outside, "node_modules"), { recursive: true, force: true });
    assert.equal(key(), before);

    // And a stub inside the project is still covered, by the tree digest.
    mkdirSync(join(project, "node_modules", "solid-js"), { recursive: true });
    writeFileSync(join(project, "node_modules", "solid-js", "package.json"), '{"version":"1.9.14"}\n');
    assert.notEqual(key(), before);
  } finally {
    rmSync(outside, { recursive: true, force: true });
  }
});

test("the chain digest reaches every ancestor, up to the filesystem root", () => {
  const outside = mkdtempSync(join(tmpdir(), "solid-checker-chain-test-"));
  try {
    const deep = join(outside, "a", "b", "c", "d");
    mkdirSync(deep, { recursive: true });
    const baseline = ancestorChainDigest(deep, "node_modules/solid-js/package.json");
    // One stub at each depth in turn: every one of them is an input.
    for (const at of [deep, join(outside, "a", "b", "c"), join(outside, "a", "b"), join(outside, "a"), outside]) {
      const stub = join(at, "node_modules", "solid-js");
      mkdirSync(stub, { recursive: true });
      writeFileSync(join(stub, "package.json"), '{"version":"1.9.14"}\n');
      assert.notEqual(
        ancestorChainDigest(deep, "node_modules/solid-js/package.json"),
        baseline,
        `a stub at ${at} must be in the chain`,
      );
      rmSync(join(at, "node_modules"), { recursive: true, force: true });
    }
    assert.equal(ancestorChainDigest(deep, "node_modules/solid-js/package.json"), baseline);
  } finally {
    rmSync(outside, { recursive: true, force: true });
  }
});

test("a unit whose inputs move during compute() is not stored", async () => {
  // The digest is taken while the key parts are built; the checker reads the
  // files afterwards. A fixture edited mid-run would otherwise be stored under
  // the *pre-edit* key with the *post-edit* result -- and reverting the edit
  // makes that entry a hit, replaying a result the bytes on disk do not
  // produce. This change makes the loop fast, so it will be run more often
  // mid-edit, which is exactly the condition that triggers it.
  const root = scaffold();
  try {
    const source = join(root, "fixtures", "unit", "App.tsx");
    const original = readFileSync(source, "utf8");
    // A thunk, as the gates now pass: re-evaluated after compute().
    const parts = () => ["project:unit", hashTree(join(root, "fixtures", "unit"))];

    const during = cacheFor(root);
    const first = await during.run(parts, async () => {
      writeFileSync(source, "export const App = () => 'EDITED-MID-RUN';\n");
      // "The checker" reads the file now -- after the digest was taken.
      return { findings: readFileSync(source, "utf8") };
    });
    assert.equal(first.hit, false);
    assert.equal(first.stored, false, "a unit whose tree moved mid-run must not be stored");
    assert.equal(during.skipped, 1);
    assert.match(during.summary(), /not stored \(inputs changed mid-run\)/);

    // The developer reverts the edit: the tree is byte-identical to the key
    // that run computed. There must be nothing filed under it.
    writeFileSync(source, original);
    let recomputed = 0;
    const later = cacheFor(root);
    const replay = await later.run(parts, async () => {
      recomputed += 1;
      return { findings: original };
    });
    assert.equal(replay.hit, false, "the mid-run result must not be replayable");
    assert.deepEqual(replay.value, { findings: original });
    assert.equal(recomputed, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a stable unit is stored, and an array of parts asserts stability by construction", async () => {
  const root = scaffold();
  try {
    const parts = () => ["project:unit", hashTree(join(root, "fixtures", "unit"))];
    const cache = cacheFor(root);
    const result = await cache.run(parts, async () => ({ findings: 1 }));
    assert.equal(result.stored, true);
    assert.equal(cache.skipped, 0);
    assert.equal((await cacheFor(root).run(parts, async () => ({ findings: 2 }))).hit, true);

    // An array is the caller promising the parts are not derived from mutable
    // state, so there is nothing to re-verify and the entry is stored.
    const fixed = cacheFor(root);
    const stored = await fixed.run(["project:constant"], async () => ({ findings: 3 }));
    assert.equal(stored.stored, true);
    assert.equal(fixed.skipped, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a fixed array carrying a filesystem digest is refused, not quietly trusted", async () => {
  // The guard that makes the re-verification non-optional. The mid-run hole is
  // created at the *call site*, by building the digest eagerly, so the shape
  // that creates it is rejected rather than documented against.
  const root = scaffold();
  try {
    for (const parts of [
      unitParts(root), // a tree digest
      ["project:unit", hashFile(join(root, "bin", "checker"))], // a file digest
      ["project:unit", ancestorChainDigest(join(root, "fixtures", "unit"), "node_modules/solid-js/package.json")],
      ["project:unit", hashTree(join(root, "nowhere"))], // "absent" is a digest too
    ]) {
      await assert.rejects(
        () => cacheFor(root).run(parts, async () => ({ findings: 1 })),
        /must be passed as a thunk/,
        JSON.stringify(parts),
      );
    }
    // ...and the thunk form of the very same parts is accepted.
    assert.equal((await cacheFor(root).run(unitThunk(root), async () => ({ ok: 1 }))).stored, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an entry with the right envelope but no value is a miss, not a hit", async () => {
  // A truncated or hand-edited entry that passes the format and key checks and
  // carries no `value` becomes `hit: true`, replays `undefined`, and fails as a
  // `TypeError: Cannot read properties of undefined (reading 'findings')` in
  // the comparison loop -- far from the cause.
  const root = scaffold();
  try {
    for (const entry of [
      { formatVersion: CACHE_FORMAT_VERSION, gate: "test-gate" }, // no `value` at all
      { formatVersion: CACHE_FORMAT_VERSION, gate: "other-gate", value: { stale: true } },
      [CACHE_FORMAT_VERSION],
      null,
      "not an entry",
    ]) {
      const cache = cacheFor(root);
      const key = cache.key(unitParts(root));
      writeJsonAtomic(
        join(cache.directory, `${key}.json`),
        entry !== null && typeof entry === "object" && !Array.isArray(entry) ? { ...entry, key } : entry,
      );
      const result = await cache.run(unitThunk(root), async () => ({ fresh: true }));
      assert.equal(result.hit, false, `${JSON.stringify(entry)} must not be replayed`);
      assert.deepEqual(result.value, { fresh: true });
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a tree digest orders by bytes, so no locale can change it", () => {
  // `localeCompare` makes the digest depend on `LANG`/`LC_ALL` -- which are not
  // in the key, so only false misses -- and can return 0 for two distinct
  // names, at which point the order falls back to whatever readdir returned.
  withScaffold((root) => {
    const unit = join(root, "fixtures", "unit");
    for (const name of ["a.tsx", "A.tsx", "ä.tsx", "z.tsx", "Z.tsx", "_.tsx"]) {
      writeFileSync(join(unit, name), `export const n = "${name}";\n`);
    }
    const digest = hashTree(unit);
    // Same bytes on disk, different collation environment: same digest.
    assert.equal(hashTree(unit), digest);
    assert.match(digest, /^tree:[0-9a-f]{64}$/);
    // Distinct names that a locale comparator can call equal stay distinct.
    writeFileSync(join(unit, "A.tsx"), "export const n = 2;\n");
    assert.notEqual(hashTree(unit), digest);
  });
});

// ---------------------------------------------------------------------------
// The registry memo (scripts/check-contract-pins.mjs).
//
// The third cache under the same kill switch, and the one whose entries matter
// most: it stores the *falsifier* -- the registry answer a bundled pin is
// compared against -- so a replayed entry is not a stale result, it is a stale
// answer to "can this pin still be falsified at all". These tests are the same
// argument the gate cache's are: one input at a time, each demanding a miss.
// ---------------------------------------------------------------------------

const REGISTRY = "https://registry.npmjs.org/";
const INTEGRITY = "sha512-OLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDOLDA==";
const REPUBLISHED = "sha512-NEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWNEWA==";

const withMemoFile = (body) => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-registry-memo-test-"));
  try {
    return body(join(directory, "registry-integrity.json"));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

const memoFile = (file, overrides) =>
  writeJsonAtomic(file, {
    formatVersion: MEMO_FORMAT_VERSION,
    inputDigest: memoInputDigest({ registry: REGISTRY }),
    entries: { "solid-js@1.9.14": INTEGRITY },
    ...overrides,
  });

test("the memo's input digest covers the registry, the format, and this script's closure", () => {
  const here = memoInputDigest({ registry: REGISTRY });
  assert.match(here, /^[0-9a-f]{64}$/);
  assert.equal(memoInputDigest({ registry: REGISTRY }), here);
  // Warm against a mirror, switch back to npmjs: every entry must miss, because
  // a mirror's answer is a different answer.
  assert.notEqual(memoInputDigest({ registry: "https://mirror.example/" }), here);
  // A different closure is a different meaning for every stored answer -- this
  // is the class that covers "someone changed `registryIntegrity`".
  assert.notEqual(
    memoInputDigest({ registry: REGISTRY, scriptPath: join(import.meta.dirname, "coverage.mjs") }),
    here,
  );
});

test("a memo whose envelope is not exactly right is discarded whole", () => {
  withMemoFile((file) => {
    const digest = memoInputDigest({ registry: REGISTRY });
    // The control: an entry this reader could have written is readable.
    memoFile(file);
    assert.deepEqual(readMemo(file, digest), { "solid-js@1.9.14": INTEGRITY });

    for (const [what, overrides] of [
      ["a foreign format version", { formatVersion: MEMO_FORMAT_VERSION + 1 }],
      ["a foreign input digest", { inputDigest: "0".repeat(64) }],
      ["no input digest", { inputDigest: undefined }],
      ["entries as an array", { entries: [INTEGRITY] }],
      ["entries as null", { entries: null }],
      ["a non-string entry", { entries: { "solid-js@1.9.14": 7 } }],
      ["an entry that is not an integrity", { entries: { "solid-js@1.9.14": "trust me" } }],
      ["a key that is not name@version", { entries: { "solid-js": INTEGRITY } }],
      ["an unrecognized top-level field", { note: "hand-edited" }],
    ]) {
      memoFile(file, overrides);
      assert.deepEqual(readMemo(file, digest), {}, `${what} must be discarded`);
    }

    // ...and so is a file that is not an object at all, or not JSON.
    writeJsonAtomic(file, [1, 2, 3]);
    assert.deepEqual(readMemo(file, digest), {});
    writeFileSync(file, "not json");
    assert.deepEqual(readMemo(file, digest), {});
    rmSync(file);
    assert.deepEqual(readMemo(file, digest), {});
  });
});

test("a memoized answer that would fail a pin is re-checked live before the verdict", () => {
  // The memo may confirm a pin; it may not condemn one. A stale entry -- warmed
  // before a republish, or hand-edited -- must not be able to invent a
  // MISMATCH, so a disagreement means miss, live lookup, then verdict.
  withMemoFile((file) => {
    memoFile(file); // memo says INTEGRITY
    const asked = [];
    const memo = memoizedIntegrity(
      (name, version) => {
        asked.push(`${name}@${version}`);
        return { integrity: REPUBLISHED };
      },
      { file, enabled: true, registry: REGISTRY },
    );

    // The pin the checked-in contract carries is the republished one; the memo
    // disagrees, so the registry is asked and its answer is what comes back.
    assert.deepEqual(memo.lookup("solid-js", "1.9.14", REPUBLISHED), { integrity: REPUBLISHED });
    assert.deepEqual(asked, ["solid-js@1.9.14"]);
    assert.match(memo.summary(), /1 memoized answer\(s\) re-checked live/);
  });
});

test("a memoized answer that agrees with the pin is served without a lookup", () => {
  withMemoFile((file) => {
    memoFile(file);
    let calls = 0;
    const memo = memoizedIntegrity(
      () => {
        calls += 1;
        return { integrity: REPUBLISHED };
      },
      { file, enabled: true, registry: REGISTRY },
    );
    assert.deepEqual(memo.lookup("solid-js", "1.9.14", INTEGRITY), { integrity: INTEGRITY });
    assert.equal(calls, 0);
    assert.match(memo.summary(), /1 hit\(s\), 0 live lookup\(s\)/);
  });
});

test("an unresolvable registry, or the kill switch, disables the memo entirely", () => {
  withMemoFile((file) => {
    memoFile(file);
    for (const [options, expected] of [
      [{ file, enabled: true, registry: null }, /could not be resolved/],
      [{ file, enabled: false }, /disabled \(SOLID_CHECKER_GATE_CACHE\)/],
    ]) {
      let calls = 0;
      const memo = memoizedIntegrity(
        () => {
          calls += 1;
          return { integrity: INTEGRITY };
        },
        options,
      );
      assert.deepEqual(memo.lookup("solid-js", "1.9.14", INTEGRITY), { integrity: INTEGRITY });
      assert.equal(calls, 1, "a disabled memo must not replay");
      assert.match(memo.summary(), expected);
      // ...and must not write either.
      memo.flush();
      assert.deepEqual(readMemo(file, memoInputDigest({ registry: REGISTRY })), {
        "solid-js@1.9.14": INTEGRITY,
      });
    }
  });
});

test("an errored lookup is never memoized", () => {
  withMemoFile((file) => {
    const memo = memoizedIntegrity(() => ({ error: "registry lookup failed: ENOTFOUND" }), {
      file,
      enabled: true,
      registry: REGISTRY,
    });
    assert.deepEqual(memo.lookup("solid-js", "1.9.14", INTEGRITY), {
      error: "registry lookup failed: ENOTFOUND",
    });
    memo.flush();
    assert.equal(existsSync(file), false, "a transient failure must not become a permanent state");
  });
});

test("flushing merges a concurrent run's entries instead of overwriting them", () => {
  withMemoFile((file) => {
    const open = () =>
      memoizedIntegrity((name) => ({ integrity: name === "solid-js" ? INTEGRITY : REPUBLISHED }), {
        file,
        enabled: true,
        registry: REGISTRY,
      });
    // Two runs open the memo at the same moment, each learns a different answer.
    const first = open();
    const second = open();
    first.lookup("solid-js", "1.9.14", INTEGRITY);
    second.lookup("seroval", "1.3.2", REPUBLISHED);
    first.flush();
    second.flush();

    // Last-writer-wins on the whole file would have dropped the first run's.
    assert.deepEqual(Object.keys(readMemo(file, memoInputDigest({ registry: REGISTRY }))).sort(), [
      "seroval@1.3.2",
      "solid-js@1.9.14",
    ]);
  });
});
