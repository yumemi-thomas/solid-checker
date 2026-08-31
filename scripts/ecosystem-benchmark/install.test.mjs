import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "vitest";
import {
  createPackageIntegrityIndex,
  packageIntegrity,
  PackageIntegrityError,
  packageIntegrityForVersion
} from "../lib/package-integrity.mjs";

import {
  buildInstallArguments,
  createProject,
  installPackages,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall,
  withTemporaryProject
} from "./lib/install.mjs";

function writeInstalledPackage(projectDir, installedPath, name, version) {
  const packageRoot = join(projectDir, ...installedPath.split("/"));
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(join(packageRoot, "package.json"), JSON.stringify({ name, version }));
  return packageRoot;
}

const hasIntegrityCode = code => error =>
  error instanceof PackageIntegrityError && error.code === code;

test("buildInstallArguments always disables lifecycle scripts and progress noise", () => {
  const args = buildInstallArguments({ specs: ["solid-js@1.9.14"] });
  assert.ok(args.includes("--ignore-scripts"), "must include --ignore-scripts (never run lifecycle scripts)");
  assert.ok(args.includes("--no-progress"), "must include --no-progress");
});

test("buildInstallArguments does not disable the lockfile, since integrity verification depends on it", () => {
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

test("readLockIntegrity reads Bun package records, including scoped packages", async () => {
  await withTemporaryProject(async dir => {
    const lock = {
      lockfileVersion: 2,
      packages: {
        "solid-js": ["solid-js@1.9.14", "", {}, "sha512-AAAA=="],
        "@solidjs/router": ["@solidjs/router@0.13.0", "", {}, "sha512-BBBB=="]
      }
    };
    mkdirSync(join(dir, "node_modules", "@solidjs", "router"), { recursive: true });
    mkdirSync(join(dir, "node_modules", "solid-js"), { recursive: true });
    writeFileSync(join(dir, "node_modules", "@solidjs", "router", "package.json"), JSON.stringify({ version: "0.13.0" }));
    writeFileSync(join(dir, "node_modules", "solid-js", "package.json"), JSON.stringify({ version: "1.9.14" }));
    writeFileSync(join(dir, "bun.lock"), `${JSON.stringify(lock, null, 2).replace(/([}\]])$/, ",\n$1")}\n`);

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

test("exact dependency integrity never aliases a different installed version", async () => {
  await withTemporaryProject(async dir => {
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}, "sha512-one"],
        "dep@2.0.0": ["dep@2.0.0", "", {}, "sha512-two"]
      }
    }));
    assert.equal(packageIntegrityForVersion(dir, "dep", "2.0.0"), "sha512-two");
    assert.equal(packageIntegrityForVersion(dir, "dep", "3.0.0"), null);
  });
});

test("Bun exact locators distinguish top-level and nested copies of the same identity", async () => {
  await withTemporaryProject(async dir => {
    const top = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    const nested = writeInstalledPackage(
      dir,
      "node_modules/parent/node_modules/dep",
      "dep",
      "1.0.0"
    );
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}, "sha512-top"],
        "parent/dep": ["dep@1.0.0", "", {}, "sha512-nested"]
      }
    }));

    const index = createPackageIntegrityIndex(dir);
    assert.equal(index.integrityForInstalledPackage(top, "dep", "1.0.0"), "sha512-top");
    assert.equal(index.integrityForInstalledPackage(nested, "dep", "1.0.0"), "sha512-nested");
    assert.equal(packageIntegrity(dir, "dep"), "sha512-top", "legacy lookup stays direct-root scoped");
    assert.throws(
      () => index.integrityForVersion("dep", "1.0.0"),
      hasIntegrityCode("ambiguous-lock-selection")
    );
  });
});

test("Bun exact-identity locators retain the published graph selector fallback", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: {
        "dep@1.0.0": ["dep@1.0.0", "", {}, "sha512-exact-locator"]
      }
    }));

    assert.equal(
      createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      "sha512-exact-locator"
    );
  });
});

test("Bun installed selection refuses an actual-locator plus exact-identity ambiguity", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}, "sha512-installed"],
        "dep@1.0.0": ["dep@1.0.0", "", {}, "sha512-exact"]
      }
    }));

    assert.throws(
      () => createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      hasIntegrityCode("ambiguous-lock-selection")
    );
  });
});

test("Bun installed selection refuses missing integrity before duplicate cardinality", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}],
        "dep@1.0.0": ["dep@1.0.0", "", {}, "sha512-exact"]
      }
    }));

    assert.throws(
      () => createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      hasIntegrityCode("missing-lock-integrity")
    );
  });
});

test("npm uses the exact package-lock packages path for top-level and nested copies", async () => {
  await withTemporaryProject(async dir => {
    const top = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    const nested = writeInstalledPackage(
      dir,
      "node_modules/parent/node_modules/dep",
      "dep",
      "1.0.0"
    );
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify({
      packages: {
        "node_modules/dep": { version: "1.0.0", integrity: "sha512-npm-top" },
        "node_modules/parent/node_modules/dep": {
          version: "1.0.0",
          integrity: "sha512-npm-nested"
        }
      }
    }));

    const index = createPackageIntegrityIndex(dir);
    assert.equal(index.integrityForInstalledPackage(top, "dep", "1.0.0"), "sha512-npm-top");
    assert.equal(index.integrityForInstalledPackage(nested, "dep", "1.0.0"), "sha512-npm-nested");
    assert.throws(
      () => index.integrityForVersion("dep", "1.0.0"),
      hasIntegrityCode("ambiguous-lock-selection")
    );
  });
});

test("an exact Bun selection takes precedence over a conflicting npm selection", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: { dep: ["dep@1.0.0", "", {}, "sha512-bun"] }
    }));
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify({
      packages: {
        "node_modules/dep": { version: "1.0.0", integrity: "sha512-stale-npm" }
      }
    }));

    assert.equal(
      createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      "sha512-bun"
    );
  });
});

test("npm exact-path fallback is allowed when Bun genuinely lacks the package identity", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: { other: ["other@1.0.0", "", {}, "sha512-other"] }
    }));
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify({
      packages: {
        "node_modules/dep": { version: "1.0.0", integrity: "sha512-npm" }
      }
    }));

    assert.equal(
      createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      "sha512-npm"
    );
  });
});

test("Bun locator disagreement refuses instead of falling through to exact npm bytes", async () => {
  await withTemporaryProject(async dir => {
    const nested = writeInstalledPackage(
      dir,
      "node_modules/parent/node_modules/dep",
      "dep",
      "1.0.0"
    );
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: { dep: ["dep@1.0.0", "", {}, "sha512-wrong-locator"] }
    }));
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify({
      packages: {
        "node_modules/parent/node_modules/dep": {
          version: "1.0.0",
          integrity: "sha512-npm"
        }
      }
    }));

    assert.throws(
      () => createPackageIntegrityIndex(dir).integrityForInstalledPackage(nested, "dep", "1.0.0"),
      hasIntegrityCode("lock-locator-mismatch")
    );
  });
});

test("Bun identity disagreement at the exact locator refuses instead of falling through to npm", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: { dep: ["dep@2.0.0", "", {}, "sha512-wrong-identity"] }
    }));
    writeFileSync(join(dir, "package-lock.json"), JSON.stringify({
      packages: {
        "node_modules/dep": { version: "1.0.0", integrity: "sha512-npm" }
      }
    }));

    assert.throws(
      () => createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      hasIntegrityCode("lock-identity-mismatch")
    );
  });
});

test("an installed root whose path disagrees with its identity is refused", async () => {
  await withTemporaryProject(async dir => {
    const root = writeInstalledPackage(dir, "node_modules/other", "other", "1.0.0");
    writeFileSync(join(dir, "bun.lock"), JSON.stringify({
      packages: { other: ["dep@1.0.0", "", {}, "sha512-wrong"] }
    }));
    assert.throws(
      () => createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      hasIntegrityCode("installed-root-mismatch")
    );
  });
});

test("an integrity index snapshots one exact lockfile for one planning transaction", async () => {
  await withTemporaryProject(async dir => {
    const lockPath = join(dir, "bun.lock");
    const root = writeInstalledPackage(dir, "node_modules/dep", "dep", "1.0.0");
    writeFileSync(lockPath, JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}, "sha512-one"]
      }
    }));
    const index = createPackageIntegrityIndex(dir);

    writeFileSync(lockPath, JSON.stringify({
      packages: {
        dep: ["dep@1.0.0", "", {}, "sha512-mutated"]
      }
    }));

    assert.equal(index.integrityForInstalledPackage(root, "dep", "1.0.0"), "sha512-one");
    assert.equal(
      createPackageIntegrityIndex(dir).integrityForInstalledPackage(root, "dep", "1.0.0"),
      "sha512-mutated",
      "an independent transaction must reread its own lockfile bytes"
    );
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

test("installPackages never spawns a real Bun process: it calls the injected spawnImpl with the built arguments", async () => {
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
