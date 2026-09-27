import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { ARCHIVES_ROOT, archiveRoots, auditedArchives } from "./audited-archives.mjs";
import { certificationEnvironment } from "./lib/certification-environment.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));

test("every audited archive installs under one root per release", () => {
  const archives = auditedArchives();
  assert.ok(archives.length > 0);
  for (const archive of archives) {
    assert.equal(archive.root, join(ARCHIVES_ROOT, archive.dialect, archive.version, "node_modules"));
    assert.equal(archive.directory, join(archive.root, archive.name));
    assert.match(archive.integrity, /^sha512-[A-Za-z0-9+/]+={0,2}$/);
    assert.match(archive.manifestSha256, /^[0-9a-f]{64}$/);
  }
});

// The citation test reads one `SOLID_CHECKER_RC<N>_ARCHIVE_ROOT` per release
// (`archive_root_variable` in solid_2.rs). A release added to
// audited-archives.json that no driver arms would only ever run the
// checked-in-slice half of that check, so every driver must name exactly the
// manifest's releases, at the roots this script installs.
test("every test driver arms exactly the manifest's archive roots", () => {
  const expected = Object.fromEntries(
    Object.entries(archiveRoots()).map(([release, rootPath]) => {
      const [, version] = release.split("@");
      const rc = /^2\.0\.0-rc\.(\d+)$/.exec(version);
      assert.ok(rc, `${release} has no archive-root variable spelling`);
      return [`SOLID_CHECKER_RC${rc[1]}_ARCHIVE_ROOT`, rootPath];
    }),
  );
  const fromEnvironment = Object.fromEntries(
    Object.entries(certificationEnvironment(root, { ...process.env, PROBE_NODE: process.execPath }))
      .filter(([name]) => /^SOLID_CHECKER_RC\d+_ARCHIVE_ROOT$/.test(name)),
  );
  assert.deepEqual(fromEnvironment, expected);

  const makefile = readFileSync(join(root, "Makefile"), "utf8");
  const verify = readFileSync(join(root, "scripts/verify.sh"), "utf8");
  for (const [variable, rootPath] of Object.entries(expected)) {
    const relative = rootPath.slice(join(ARCHIVES_ROOT, "solid-v2").length + 1);
    assert.ok(
      makefile.includes(`${variable}="$(ARCHIVES_ROOT)/${relative}"`),
      `Makefile ARCHIVE_ENV does not arm ${variable}`,
    );
    assert.ok(verify.includes(`${variable}="$archives_root/${relative}"`), `verify.sh does not arm ${variable}`);
  }
  assert.equal((makefile.match(/SOLID_CHECKER_RC\d+_ARCHIVE_ROOT=/g) ?? []).length, Object.keys(expected).length);
});
