import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import { assertManifestProvenance } from "./lib/corpus-manifest.mjs";

const commit = "35c4cc6b7c932f5dee5815b37d3601e7dff90b58";
const manifest = () => ({
  apps: { one: { repository: "https://example.test/one", commit } },
  projects: [
    { app: "one", root: "one", project: "tsconfig.json" },
    { app: "one", root: "one", project: "tsconfig.node.json" },
  ],
});

test("a manifest whose apps carry a repository and commit is accepted", () => {
  assertManifestProvenance(manifest());
});

test("an app without a repository is refused by name", () => {
  const broken = manifest();
  delete broken.apps.one.repository;
  assert.throws(() => assertManifestProvenance(broken, "m.json"), /m\.json.*app one has no repository/);
});

test("an app without a commit, or with a branch name for one, is refused by name", () => {
  const missing = manifest();
  delete missing.apps.one.commit;
  assert.throws(() => assertManifestProvenance(missing), /app one has no commit/);
  const branch = manifest();
  branch.apps.one.commit = "main";
  assert.throws(() => assertManifestProvenance(branch), /app one commit is not a full commit hash/);
});

test("a project row whose app has no provenance entry is refused", () => {
  const broken = manifest();
  broken.projects.push({ app: "two", root: "two", project: "tsconfig.json" });
  assert.throws(() => assertManifestProvenance(broken), /app two has no repository.*app two has no commit/);
  assert.throws(() => assertManifestProvenance({ projects: broken.projects }), /app one has no repository/);
});

test("the committed rc.17 manifest has provenance for every app it sweeps", () => {
  const committed = JSON.parse(readFileSync(new URL("./corpus/rc17-manifest.json", import.meta.url), "utf8"));
  assertManifestProvenance(committed, "rc17-manifest.json");
  assert.ok(!committed.projects.some((row) => row.app === "lutra-console"));
  assert.deepEqual(committed.excluded.unavailable.apps, ["lutra-console"]);
  assert.match(committed.rebuilt.comparability, /not comparable/);
});
