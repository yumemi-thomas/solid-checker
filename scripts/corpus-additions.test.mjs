// What was added to the corpus checkouts (scripts/lib/corpus-additions.mjs): the record, its check against the corpus, and the corpus left otherwise as it was.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { assertManifestProvenance } from "./lib/corpus-manifest.mjs";
import { assertManifestAdditions, changedSince, checkAdditions, filesUnder, hashTree, recordedOutputs } from "./lib/corpus-additions.mjs";

const commit = "35c4cc6b7c932f5dee5815b37d3601e7dff90b58";
const write = (root, path, text) => { mkdirSync(join(root, path, ".."), { recursive: true }); writeFileSync(join(root, path), text); };

/** A corpus of one app with a generated `dist` of two files, and the manifest that records it. */
function corpus() {
  const root = mkdtempSync(join(tmpdir(), "corpus-additions-"));
  write(root, "one/package.json", "{}");
  write(root, "one/src/index.ts", "export {};");
  write(root, "one/packages/lib/dist/index.js", "export const a = 1;\n");
  write(root, "one/packages/lib/dist/util/b.js", "export const b = 2;\n");
  write(root, "one/node_modules/tool/cache.json", "{}");
  const tree = hashTree(join(root, "one/packages/lib/dist"));
  const manifest = {
    apps: {
      one: {
        repository: "https://example.test/one",
        commit,
        additions: [{
          id: "workspace-dist-lib", kind: "workspace-dist", cwd: ".", commands: ["vp run --filter lib build"],
          tools: [{ name: "vite-plus", version: "0.2.9" }], output: "packages/lib/dist", files: tree.files, sha256: tree.sha256, result: { exit: 0 },
        }],
      },
    },
    projects: [{ app: "one", root: "one", project: "tsconfig.json" }],
  };
  return { root, manifest };
}

test("a directory's record is its file count and a digest of every file's digest and path, in path order", () => {
  const { root } = corpus();
  const dist = join(root, "one/packages/lib/dist");
  assert.deepEqual(filesUnder(dist), ["index.js", "util/b.js"]);
  const first = hashTree(dist);
  assert.equal(first.files, 2);
  assert.match(first.sha256, /^[0-9a-f]{64}$/);
  assert.deepEqual(hashTree(dist), first);
  writeFileSync(join(dist, "util/b.js"), "export const b = 3;\n");
  assert.notEqual(hashTree(dist).sha256, first.sha256);
  rmSync(root, { recursive: true, force: true });
});

test("an addition names its kind, commands, tools with exact versions, output, file count, digest and how the build ended, or the manifest is refused", () => {
  const { root, manifest } = corpus();
  assert.doesNotThrow(() => assertManifestAdditions(manifest));
  assert.doesNotThrow(() => assertManifestProvenance(manifest));
  const broken = (change) => { const copy = structuredClone(manifest); change(copy.apps.one.additions[0]); return copy; };
  assert.throws(() => assertManifestAdditions(broken((a) => { a.kind = "build"; }), "m.json"), /m\.json.*app one addition workspace-dist-lib: kind is not one of/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.commands = []; })), /no commands/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.tools = [{ name: "vp", version: "" }]; })), /a tool without a name and an exact version/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.output = "/abs/dist"; })), /no relative output directory/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.output = "../dist"; })), /no relative output directory/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.sha256 = "abc"; })), /no sha256/);
  assert.throws(() => assertManifestAdditions(broken((a) => { delete a.result; })), /no result/);
  assert.throws(() => assertManifestAdditions(broken((a) => { a.files = 0; })), /no file count/);
  const twice = structuredClone(manifest);
  twice.apps.one.additions.push(structuredClone(twice.apps.one.additions[0]));
  assert.throws(() => assertManifestAdditions(twice), /id repeated/);
  assert.throws(() => assertManifestProvenance(broken((a) => { a.commands = []; })), /not recorded in full/);
  rmSync(root, { recursive: true, force: true });
});

test("an output the corpus lacks, or holds with another count or digest, is a problem; the record matches the corpus it was made from", () => {
  const { root, manifest } = corpus();
  assert.deepEqual(checkAdditions({ manifest, appsRoot: root }), []);
  write(root, "one/packages/lib/dist/extra.js", "x");
  assert.match(checkAdditions({ manifest, appsRoot: root })[0], /holds 3 files, the record says 2/);
  rmSync(join(root, "one/packages/lib/dist/extra.js"));
  writeFileSync(join(root, "one/packages/lib/dist/index.js"), "export const a = 9;\n");
  assert.match(checkAdditions({ manifest, appsRoot: root })[0], /another digest than the record/);
  rmSync(join(root, "one/packages/lib/dist"), { recursive: true });
  assert.match(checkAdditions({ manifest, appsRoot: root })[0], /workspace-dist-lib: one\/packages\/lib\/dist is missing/);
  assert.deepEqual(recordedOutputs(manifest).map(({ dir }) => dir), ["one/packages/lib/dist"]);
  rmSync(root, { recursive: true, force: true });
});

test("a file of the corpus newer than the marker that lies under no recorded output is a change the record does not name; node_modules and .git are not looked at", () => {
  const { root, manifest } = corpus();
  // the checkout as it was: every file old; then the marker; then what generation makes
  const old = new Date(Date.now() - 7_200_000);
  for (const path of filesUnder(root)) utimesSync(join(root, path), old, old);
  const marker = join(root, "marker");
  writeFileSync(marker, "");
  const stamp = new Date(Date.now() - 3_600_000);
  utimesSync(marker, stamp, stamp);
  write(root, "one/packages/lib/dist/index.js", "export const a = 1;\n");
  write(root, "one/packages/lib/dist/util/b.js", "export const b = 2;\n");
  write(root, "one/node_modules/tool/cache.json", "{\"touched\":true}");
  write(root, ".git/x", "1");
  const changed = changedSince(root, marker);
  assert.deepEqual(changed, ["one/packages/lib/dist/index.js", "one/packages/lib/dist/util/b.js"]);
  assert.deepEqual(checkAdditions({ manifest, appsRoot: root, changed }), []);
  write(root, "one/src/index.ts", "export const changed = true;");
  assert.deepEqual(checkAdditions({ manifest, appsRoot: root, changed: changedSince(root, marker) }), ["changed outside the recorded additions: one/src/index.ts"]);
  rmSync(root, { recursive: true, force: true });
});

test("the committed rc.21 manifest records every addition in full: Paraglide for two apps, Panda for one, and seven workspace packages' dist, each from an exact tool version", () => {
  const committed = JSON.parse(readFileSync(new URL("./corpus/rc21-manifest.json", import.meta.url), "utf8"));
  assertManifestProvenance(committed, "rc21-manifest.json");
  const all = recordedOutputs(committed);
  assert.deepEqual(all.map(({ app, id }) => `${app}/${id}`).sort(), [
    "ai-memory-ui/paraglide",
    "hoxxes-briefing/panda-codegen",
    "probus-hk/paraglide",
    "viviana-ui-web/workspace-dist-geist",
    "viviana-ui-web/workspace-dist-kumo",
    "viviana-ui-web/workspace-dist-solid-spectrum",
    "viviana-ui-web/workspace-dist-solid-stately",
    "viviana-ui-web/workspace-dist-solidaria",
    "viviana-ui-web/workspace-dist-solidaria-components",
    "viviana-ui-web/workspace-dist-viviana-ui",
  ]);
  for (const { addition } of all) for (const tool of addition.tools) assert.match(tool.version, /^\d+\.\d+\.\d+/, `${addition.id}: ${tool.name}`);
  // a Paraglide compile names the message-format plugin it ran with by exact version and integrity: the checkouts' own settings name a floating CDN URL
  for (const { app } of all.filter(({ addition }) => addition.kind === "paraglide")) {
    const plugin = committed.apps[app].additions[0].tools.find((tool) => tool.name === "@inlang/plugin-message-format");
    assert.equal(plugin.version, "4.6.0");
    assert.match(plugin.integrity, /^sha512-/);
  }
  // a build that ended with type errors says so and says what that left
  const failed = all.filter(({ addition }) => addition.result.exit !== 0);
  assert.deepEqual(failed.map(({ id }) => id).sort(), ["workspace-dist-solidaria", "workspace-dist-solidaria-components"]);
  for (const { addition } of failed) assert.ok(addition.result.tscErrors > 0 && /emitted/.test(addition.result.note));
});
