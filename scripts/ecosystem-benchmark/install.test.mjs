import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import {
  buildInstallArguments,
  createProject,
  installPackages,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall,
  withTemporaryProject
} from "./lib/install.mjs";

test("buildInstallArguments always disables lifecycle scripts, audit, and fund", () => {
  const args = buildInstallArguments({ specs: ["solid-js@1.9.14"] });
  assert.ok(args.includes("--ignore-scripts"), "must include --ignore-scripts (never run lifecycle scripts)");
  assert.ok(args.includes("--no-audit"), "must include --no-audit");
  assert.ok(args.includes("--no-fund"), "must include --no-fund");
});

test("buildInstallArguments never disables the lockfile, since integrity verification depends on it", () => {
  const args = buildInstallArguments({ specs: ["solid-js@1.9.14"] });
  assert.ok(!args.includes("--no-package-lock"), "must NOT include --no-package-lock");
});

test("buildInstallArguments includes install and the requested specs", () => {
  const args = buildInstallArguments({ specs: ["solid-js@1.9.14", "@solidjs/router@0.13.0"] });
  assert.equal(args[0], "install");
  assert.ok(args.includes("solid-js@1.9.14"));
  assert.ok(args.includes("@solidjs/router@0.13.0"));
});

test("createProject writes a package.json marked private", async () => {
  await withTemporaryProject(async dir => {
    await createProject({ root: dir, specs: ["solid-js@1.9.14"] });
    const pkgPath = join(dir, "package.json");
    assert.ok(existsSync(pkgPath));
    const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
    assert.equal(pkg.private, true);
  });
});

test("readInstalledVersions reads version from an installed package's package.json, including scoped packages", async () => {
  await withTemporaryProject(async dir => {
    const scopedDir = join(dir, "node_modules", "@solidjs", "router");
    const plainDir = join(dir, "node_modules", "solid-js");
    mkdirSync(scopedDir, { recursive: true });
    mkdirSync(plainDir, { recursive: true });
    writeFileSync(join(scopedDir, "package.json"), JSON.stringify({ name: "@solidjs/router", version: "0.13.0" }));
    writeFileSync(join(plainDir, "package.json"), JSON.stringify({ name: "solid-js", version: "1.9.14" }));

    const versions = readInstalledVersions(dir, ["@solidjs/router", "solid-js", "missing-pkg"]);
    assert.equal(versions["@solidjs/router"], "0.13.0");
    assert.equal(versions["solid-js"], "1.9.14");
    assert.equal(versions["missing-pkg"], null);
  });
});

test("readLockIntegrity reads packages[node_modules/<name>].integrity from a written package-lock.json", async () => {
  await withTemporaryProject(async dir => {
    const lock = {
      name: "probe",
      lockfileVersion: 3,
      packages: {
        "": { name: "probe" },
        "node_modules/solid-js": { version: "1.9.14", integrity: "sha512-AAAA==" },
        "node_modules/@solidjs/router": { version: "0.13.0", integrity: "sha512-BBBB==" }
      }
    };
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify(lock));

    const integrity = readLockIntegrity(dir, ["solid-js", "@solidjs/router", "missing-pkg"]);
    assert.equal(integrity["solid-js"], "sha512-AAAA==");
    assert.equal(integrity["@solidjs/router"], "sha512-BBBB==");
    assert.equal(integrity["missing-pkg"], null);
  });
});

test("readLockIntegrity returns null for every name when there is no lockfile at all", async () => {
  await withTemporaryProject(async dir => {
    const integrity = readLockIntegrity(dir, ["solid-js"]);
    assert.equal(integrity["solid-js"], null);
  });
});

test("verifyInstall reports a missing package", () => {
  const { ok, problems } = verifyInstall({
    expected: { "solid-js": { version: "1.9.14", integrity: "sha512-AAAA==" } },
    versions: { "solid-js": null },
    integrity: { "solid-js": null }
  });
  assert.equal(ok, false);
  assert.equal(problems.length, 1);
  assert.equal(problems[0].kind, "missing");
  assert.equal(problems[0].package, "solid-js");
});

test("verifyInstall reports a version mismatch", () => {
  const { ok, problems } = verifyInstall({
    expected: { "solid-js": { version: "1.9.14", integrity: "sha512-AAAA==" } },
    versions: { "solid-js": "1.9.13" },
    integrity: { "solid-js": "sha512-AAAA==" }
  });
  assert.equal(ok, false);
  assert.equal(problems.length, 1);
  assert.equal(problems[0].kind, "version-mismatch");
  assert.equal(problems[0].expectedVersion, "1.9.14");
  assert.equal(problems[0].actualVersion, "1.9.13");
});

test("verifyInstall reports an integrity mismatch and never ignores it, even when the version matches", () => {
  const { ok, problems } = verifyInstall({
    expected: { "solid-js": { version: "1.9.14", integrity: "sha512-AAAA==" } },
    versions: { "solid-js": "1.9.14" },
    integrity: { "solid-js": "sha512-ZZZZ==" }
  });
  assert.equal(ok, false);
  const integrityProblems = problems.filter(p => p.kind === "integrity-mismatch");
  assert.equal(integrityProblems.length, 1, "an integrity mismatch must always be reported");
  assert.equal(integrityProblems[0].expectedIntegrity, "sha512-AAAA==");
  assert.equal(integrityProblems[0].actualIntegrity, "sha512-ZZZZ==");
});

test("verifyInstall reports ok with no problems when everything matches", () => {
  const { ok, problems } = verifyInstall({
    expected: { "solid-js": { version: "1.9.14", integrity: "sha512-AAAA==" } },
    versions: { "solid-js": "1.9.14" },
    integrity: { "solid-js": "sha512-AAAA==" }
  });
  assert.equal(ok, true);
  assert.deepEqual(problems, []);
});

test("withTemporaryProject removes its directory after a successful callback", async () => {
  let capturedDir;
  await withTemporaryProject(async dir => {
    capturedDir = dir;
    assert.ok(existsSync(dir));
  });
  assert.ok(!existsSync(capturedDir), "temporary directory must be removed after use");
});

test("withTemporaryProject removes its directory even when the callback throws", async () => {
  let capturedDir;
  await assert.rejects(
    withTemporaryProject(async dir => {
      capturedDir = dir;
      assert.ok(existsSync(dir));
      throw new Error("boom");
    }),
    /boom/
  );
  assert.ok(capturedDir, "callback must have run");
  assert.ok(!existsSync(capturedDir), "temporary directory must still be removed after a throw");
});

test("installPackages never spawns a real npm process: it calls the injected spawnImpl with the built arguments", async () => {
  const calls = [];
  const fakeSpawn = async ({ cwd, args, timeoutMs }) => {
    calls.push({ cwd, args, timeoutMs });
    return { status: 0, stdout: "added 1 package", stderr: "", timedOut: false };
  };

  const result = await installPackages({
    projectDir: "/tmp/does-not-need-to-exist-for-this-fake",
    specs: ["solid-js@1.9.14"],
    spawnImpl: fakeSpawn,
    timeoutMs: 5000
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].cwd, "/tmp/does-not-need-to-exist-for-this-fake");
  assert.ok(calls[0].args.includes("--ignore-scripts"));
  assert.ok(calls[0].args.includes("solid-js@1.9.14"));
  assert.equal(calls[0].timeoutMs, 5000);
  assert.equal(result.status, 0);
});

test("installPackages surfaces a fake timeout without touching the network", async () => {
  const fakeSpawn = async () => ({ status: null, stdout: "", stderr: "", timedOut: true });
  const result = await installPackages({
    projectDir: "/tmp/does-not-need-to-exist-for-this-fake",
    specs: ["solid-js@1.9.14"],
    spawnImpl: fakeSpawn
  });
  assert.equal(result.timedOut, true);
});
