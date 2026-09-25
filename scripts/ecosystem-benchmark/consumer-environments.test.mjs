// Consumer environments are reviewed data a delivery run trusts, so what is
// tested is what they refuse: a package that is not its manifest row's artifact,
// a runtime no audit read, and a runtime above the audited Solid 2 release.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  consumerEnvironmentManifest,
  deriveConsumerEnvironment,
  environmentProblems,
  environmentProbeId,
  loadConsumerEnvironments
} from "./lib/consumer-environments.mjs";
import { loadAuditedArchives } from "./lib/dialect-authority.mjs";
import { AUDITED_SOLID_2 } from "./lib/families.mjs";
import { consumerEnvironmentConflicts, probeInstallPlan, runBenchmark } from "./run.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const manifest = JSON.parse(readFileSync(join(ROOT, "scripts/ecosystem-benchmark/manifest.json"), "utf8"));
const auditedArchives = loadAuditedArchives();
const reviewed = () => structuredClone(loadConsumerEnvironments().environments);
const kobalte = () => reviewed().find(entry => entry.id === "kobalte-solid2-e9d426d4");

test("the reviewed kobalte environment is admissible against the committed manifest and audits", () => {
  const environment = kobalte();
  assert.ok(environment, "the kobalte solid2 environment is listed");
  assert.deepEqual(environmentProblems(environment, { manifest, auditedArchives }), []);
  assert.equal(environment.source.commit, "e9d426d438b7c9ea0cc81bd1133831a20cd5fcae");
  assert.equal(environment.source.branch, "solid2");
  for (const name of ["solid-js", "@solidjs/web", "@solidjs/signals"]) {
    assert.equal(environment.runtime[name].version, "2.0.0-rc.3");
  }
  const names = environment.packages.map(entry => entry.package);
  for (const tier of ["keyed", "platform", "rootless", "trigger", "utils"]) {
    assert.ok(names.includes(`@solid-primitives/${tier}`), `${tier} is delivered in kobalte's tree`);
  }
  // The two kobalte installs at another release than the manifest row are
  // recorded, not certified.
  assert.deepEqual(
    environment.unmatched.map(entry => `${entry.package}@${entry.version}`),
    ["@solid-primitives/event-listener@3.0.0-next.5", "@solid-primitives/form@1.0.0-next.3"]
  );
  for (const entry of environment.unmatched) assert.ok(!names.includes(entry.package));
});

test("every listed package becomes one environment probe cloned from its manifest row", () => {
  const environment = kobalte();
  const derived = consumerEnvironmentManifest(manifest, environment, { auditedArchives });
  assert.equal(derived.rows.length, environment.packages.length);
  assert.deepEqual(derived.supplemental, []);
  for (const row of derived.rows) {
    const source = manifest.rows.find(candidate => candidate.solidTarget === "solid2" && candidate.package === row.package);
    assert.equal(row.version, source.version);
    assert.equal(row.integrity, source.integrity);
    assert.equal(row.probes.length, 1);
    const [probe] = row.probes;
    assert.equal(probe.id, environmentProbeId(row, environment));
    assert.equal(probe.kind, "environment");
    assert.deepEqual(probe.solid, { "solid-js": "2.0.0-rc.3", "@solidjs/web": "2.0.0-rc.3", "@solidjs/signals": "2.0.0-rc.3" });

    const { specs, pins, overrides } = probeInstallPlan(row, probe, manifest.solidReleases);
    assert.deepEqual(specs.slice(0, 1), [`${row.package}@${row.version}`]);
    assert.equal(pins["@solidjs/signals"].required, true);
    assert.equal(pins["@solidjs/signals"].resolvedFrom, "solid-js");
    assert.equal(pins["@solidjs/signals"].integrity, environment.runtime["@solidjs/signals"].integrity);
    assert.equal(pins.seroval.required, false, "a closure pin is held only where it is installed");
    assert.equal(overrides.seroval, environment.pins.seroval.version);
    assert.equal(overrides["@solid-primitives/event-listener"], "3.0.0-next.5", "the consumer's own release, not the manifest's");
  }
  // The committed manifest is untouched.
  assert.ok(manifest.rows.every(row => row.probes.every(probe => probe.kind !== "environment")));
});

test("a package that is not its manifest row's artifact is refused", () => {
  const version = kobalte();
  version.packages[0].version = "9.9.9";
  assert.match(environmentProblems(version, { manifest, auditedArchives }).join("\n"), /is not the manifest row's/);

  const bytes = kobalte();
  bytes.packages[0].integrity = "sha512-AAAA";
  assert.match(environmentProblems(bytes, { manifest, auditedArchives }).join("\n"), /integrity .* is not the manifest row's/);

  const unknown = kobalte();
  unknown.packages.push({ package: "@example/nowhere", version: "1.0.0", integrity: "sha512-AAAA" });
  assert.match(environmentProblems(unknown, { manifest, auditedArchives }).join("\n"), /has no solid2 manifest row/);

  const conflicting = kobalte();
  conflicting.pins["@solid-primitives/utils"] = { version: "7.0.0-next.5", integrity: "sha512-AAAA" };
  assert.match(environmentProblems(conflicting, { manifest, auditedArchives }).join("\n"), /is pinned at 7\.0\.0-next\.5/);

  assert.throws(() => consumerEnvironmentManifest(manifest, bytes, { auditedArchives }), /refused:/);
});

test("a runtime above the audited Solid 2 release, or not an audited archive, is refused", () => {
  const newer = kobalte();
  newer.runtime["solid-js"] = { version: "2.0.0-rc.9", integrity: "sha512-AAAA" };
  newer.runtime["@solidjs/web"] = { version: "2.0.0-rc.9", integrity: "sha512-AAAA" };
  const problems = environmentProblems(newer, { manifest, auditedArchives }).join("\n");
  assert.match(problems, new RegExp(`solid-js@2\\.0\\.0-rc\\.9 is above the audited Solid 2 release ${AUDITED_SOLID_2.replace(/\./g, "\\.")}`));
  assert.match(problems, /@solidjs\/web@2\.0\.0-rc\.9 is above/);

  const republished = kobalte();
  republished.runtime["@solidjs/signals"].integrity = "sha512-AAAA";
  assert.match(environmentProblems(republished, { manifest, auditedArchives }).join("\n"), /is not the audited archive's/);

  const unaudited = kobalte();
  unaudited.runtime["@solidjs/signals"] = { version: "2.0.0-rc.9", integrity: "sha512-AAAA" };
  assert.match(environmentProblems(unaudited, { manifest, auditedArchives }).join("\n"), /@solidjs\/signals@2\.0\.0-rc\.9 is not an audited archive/);

  const missing = kobalte();
  delete missing.runtime["@solidjs/web"];
  assert.match(environmentProblems(missing, { manifest, auditedArchives }).join("\n"), /runtime @solidjs\/web needs/);
});

test("the environment is the whole selection of a delivery run", () => {
  assert.deepEqual(consumerEnvironmentConflicts({ consumerEnvironment: null, packages: ["x"] }), []);
  assert.deepEqual(consumerEnvironmentConflicts({ consumerEnvironment: "e", solidTargets: ["2"] }), []);
  assert.deepEqual(
    consumerEnvironmentConflicts({
      consumerEnvironment: "e",
      sentinel: true,
      families: ["kobalte"],
      probeIds: ["p"],
      packages: ["x"],
      includeSupplemental: true,
      solidTargets: ["1"]
    }),
    ["--sentinel", "--family", "--probe", "--package", "--include-supplemental", "--solid other than 2"]
  );
});

test("an environment probe's runtime and pins reach the install, and a wrong copy refuses it", async () => {
  const derived = consumerEnvironmentManifest(manifest, kobalte(), { auditedArchives });
  derived.rows = derived.rows.filter(row => row.package === "@solid-primitives/keyed");
  const installs = [];
  const hooks = signalsCopy => ({
    now: () => 0,
    mkProject: async () => ({ projectDir: "/tmp/project", outputDir: "/tmp/out" }),
    installPackages: async args => {
      installs.push(args);
      return {
        status: 0,
        stdout: "",
        stderr: "",
        timedOut: false,
        installedVersions: Object.fromEntries(Object.entries(args.expected).map(([name, want]) => [name, want.version])),
        integrity: Object.fromEntries(Object.entries(args.expected).map(([name, want]) => [name, want.integrity])),
        pinned: {
          lockRead: true,
          copies: Object.fromEntries(
            Object.entries(args.pins).map(([name, pin]) => [
              name,
              name === "@solidjs/signals" ? [signalsCopy(pin)] : pin.required ? [{ locator: name, version: pin.version, integrity: pin.integrity }] : []
            ])
          ),
          resolutions: {
            "@solidjs/signals": { from: "solid-js", fromInstalled: true, path: "/s", hoistedPath: "/s", version: args.pins["@solidjs/signals"].version }
          }
        }
      };
    },
    generateContract: async () => ({ status: 0, stdout: "generated pkg contract with 1 entrypoints", stderr: "", timedOut: false }),
    cleanup: async () => {}
  });

  const [clean] = await runBenchmark({
    manifest: derived,
    hooks: hooks(pin => ({ locator: "@solidjs/signals", version: pin.version, integrity: pin.integrity }))
  });
  assert.equal(installs[0].overrides["@solidjs/signals"], "2.0.0-rc.3");
  assert.equal(installs[0].overrides["@solid-primitives/utils"], "7.0.0-next.4");
  assert.equal(clean.probeKind, "environment");
  assert.equal(clean.integrityVerified, true);

  const [rc6] = await runBenchmark({
    manifest: derived,
    hooks: hooks(() => ({ locator: "@solidjs/signals", version: "2.0.0-rc.6", integrity: "sha512-rc6" }))
  });
  assert.equal(rc6.outcome, "failure");
  assert.equal(rc6.class, "install-failure");
  assert.match(rc6.signature, /version-mismatch: @solidjs\/signals/);
});

test("an entry derived from a pnpm lock takes the consumer's closure and names what it leaves out", () => {
  const utils = manifest.rows.find(row => row.solidTarget === "solid2" && row.package === "@solid-primitives/utils");
  const keyed = manifest.rows.find(row => row.solidTarget === "solid2" && row.package === "@solid-primitives/keyed");
  const peer = "(@solidjs/web@2.0.0-rc.3(solid-js@2.0.0-rc.3))(solid-js@2.0.0-rc.3)";
  const archive = name => auditedArchives.dialects[0].archives.find(entry => entry.name === name && entry.version === "2.0.0-rc.3").integrity;
  const lock = {
    importers: {
      "packages/core": {
        dependencies: {
          "@solid-primitives/keyed": { specifier: keyed.version, version: `${keyed.version}${peer}` },
          "@solid-primitives/form": { specifier: "1.0.0-next.3", version: `1.0.0-next.3${peer}` },
          "@kobalte/utils": { specifier: "workspace:*", version: "link:../utils" }
        }
      }
    },
    packages: {
      [`@solid-primitives/keyed@${keyed.version}`]: { resolution: { integrity: keyed.integrity } },
      [`@solid-primitives/utils@${utils.version}`]: { resolution: { integrity: utils.integrity } },
      "@solid-primitives/form@1.0.0-next.3": { resolution: { integrity: "sha512-form" } },
      "solid-js@1.9.14": { resolution: { integrity: "sha512-one" } },
      "solid-js@2.0.0-rc.3": { resolution: { integrity: archive("solid-js") } },
      "@solidjs/web@2.0.0-rc.3": { resolution: { integrity: archive("@solidjs/web") } },
      "@solidjs/signals@2.0.0-rc.3": { resolution: { integrity: archive("@solidjs/signals") } },
      "seroval@1.5.4": { resolution: { integrity: "sha512-seroval" } }
    },
    snapshots: {
      [`@solid-primitives/keyed@${keyed.version}${peer}`]: { dependencies: { "@solid-primitives/utils": `${utils.version}${peer}` } },
      [`@solid-primitives/utils@${utils.version}${peer}`]: { dependencies: { "solid-js": "2.0.0-rc.3" } },
      [`@solid-primitives/form@1.0.0-next.3${peer}`]: {},
      "solid-js@2.0.0-rc.3": { dependencies: { "@solidjs/signals": "2.0.0-rc.3", seroval: "1.5.4" } },
      "@solidjs/web@2.0.0-rc.3(solid-js@2.0.0-rc.3)": { dependencies: { seroval: "1.5.4" } },
      "@solidjs/signals@2.0.0-rc.3": {},
      "seroval@1.5.4": {}
    }
  };
  const entry = deriveConsumerEnvironment({
    lock,
    id: "example",
    source: { repository: "https://example.invalid/x", branch: "main", commit: "0".repeat(40), lockfile: "pnpm-lock.yaml", lockfileDigest: `sha256:${"0".repeat(64)}` },
    importers: ["packages/core"],
    manifest
  });
  assert.deepEqual(entry.packages.map(item => item.package), ["@solid-primitives/keyed", "@solid-primitives/utils"]);
  assert.deepEqual(Object.keys(entry.pins), ["@solid-primitives/keyed", "@solid-primitives/utils", "seroval"]);
  assert.deepEqual(entry.unmatched.map(item => item.package), ["@solid-primitives/form"]);
  assert.equal(entry.runtime["solid-js"].version, "2.0.0-rc.3", "the Solid 2 release, not the docs app's 1.x");
  assert.deepEqual(environmentProblems(entry, { manifest, auditedArchives }), []);
});
