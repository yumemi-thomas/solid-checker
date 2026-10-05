// The authored contract tier (ADR 0198): probe its claims, then build it.
//
//   bun scripts/author-contracts.mjs probe <chromium> --only <spec> --install <dir>
//       run one spec's misuse/correct pairs in an install that holds the
//       package at its version on the spec's Solid runtime; record the results
//       in probe-results.json
//   bun scripts/author-contracts.mjs build
//       write pkg/contracts/authored/{index.json,objects/**} and embedded.rs
//   bun scripts/author-contracts.mjs check
//       fail if the written tier is not what `build` would write
//
// A spec is a directory `pkg/contracts/authored/specs/<package>@<version>/`
// holding `spec.json` and, per claimed export, `<export>.misuse.tsx` and
// `<export>.correct.tsx`. `spec.json` names the package version, the Solid
// runtime its claims are probed on, and per export the authored `call` and the
// misuse rule its pair exercises.
//
// Each artifact case of that version starts from the certified document the
// compiled-in tier already carries for it: its identity, case structure and
// file digests are the version's own. Every export's `call` is then replaced,
// with the authored claim where the spec states one and its pair passed, and
// fully open (`{}`) everywhere else. Nothing a certification inferred ships
// without its own probe.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TIER = join(ROOT, "pkg/contracts/authored");
const ACCEPTED = join(ROOT, "pkg/contracts/accepted");
const EMBEDDED = join(ROOT, "rust/crates/solid-facts-backend/src/authored_contracts/embedded.rs");
const RESULTS = join(TIER, "probe-results.json");
const LEDGER = join(ROOT, "benchmarks/reviewed-package-models/development/misuse-runtime-ledger.mjs");
// The case-level fields a certified document may carry. Anything else is a
// claim this script would carry without a probe, so it refuses.
const CASE_FIELDS = new Set(["artifact", "declarations", "resolution", "exports"]);

const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const json = value => `${JSON.stringify(value, null, 2)}\n`;
const read = path => JSON.parse(readFileSync(path, "utf8"));
const [command, ...rest] = process.argv.slice(2);
const option = name => { const index = rest.indexOf(name); return index >= 0 ? rest[index + 1] : undefined; };

const specs = readdirSync(join(TIER, "specs")).sort().map(name => {
  const directory = join(TIER, "specs", name);
  const spec = read(join(directory, "spec.json"));
  assert.equal(name, `${spec.package.replace("/", "+")}@${spec.version}`, `${name}: directory names another version`);
  return { ...spec, name, directory };
});
const accepted = read(join(ACCEPTED, "index.json"));

/** The certified cases of one version: one per (entrypoint, conditions, target). */
function certifiedCases(spec) {
  const cases = new Map();
  for (const bundle of accepted.bundles) {
    if (bundle.packageName !== spec.package || bundle.packageVersion !== spec.version) continue;
    const key = JSON.stringify([bundle.specifier, bundle.requestedEntrypoint, bundle.exportConditions, bundle.runtimeTarget]);
    if (!cases.has(key)) cases.set(key, bundle);
  }
  assert(cases.size > 0, `${spec.name}: the certified tier carries no case of this version`);
  return [...cases.values()];
}

/** The installed directory of `name` as Node resolves it from `from`. */
function installed(name, from) {
  for (let directory = from; ; directory = dirname(directory)) {
    const candidate = join(directory, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return realpathSync(candidate);
    assert.notEqual(directory, dirname(directory), `${name} is not installed above ${from}`);
  }
}

/**
 * The identity a probe ran on: the package's own files reproduce one certified
 * case's artifact, and the Solid runtime it resolves is the spec's, by version.
 */
function probeInstall(spec, install) {
  const packageDirectory = installed(spec.package, install);
  const manifest = read(join(packageDirectory, "package.json"));
  assert.equal(manifest.version, spec.version, `${spec.name}: the probe install holds ${manifest.version}`);
  const artifacts = certifiedCases(spec).map(bundle => {
    const document = read(join(ACCEPTED, bundle.document));
    const [artifactCase] = Object.values(document.entrypoints).flatMap(entrypoint => entrypoint.cases ?? [entrypoint]);
    const path = artifactCase.artifact.path.replace(/^\.\//, "");
    const bytes = readFileSync(join(packageDirectory, path));
    assert.equal(sha256(bytes), artifactCase.artifact.sha256, `${spec.name}: the probe install's ${path} is not the published file`);
    return path;
  });
  const runtime = spec.solidRuntime.map(entry => {
    const directory = installed(entry.name, packageDirectory);
    assert.equal(read(join(directory, "package.json")).version, entry.version, `${spec.name}: the probe install resolves another ${entry.name}`);
    return { name: entry.name, version: entry.version };
  });
  return { install, artifacts, runtime };
}

function probe(browser) {
  const only = option("--only"), install = option("--install");
  assert(browser && only && install, "usage: author-contracts.mjs probe <chromium> --only <spec> --install <dir>");
  const results = (existsSync(RESULTS) ? read(RESULTS).results : []).filter(row => row.spec !== only);
  for (const spec of specs) {
    if (spec.name !== only) continue;
    const identity = probeInstall(spec, realpathSync(install));
    const cases = Object.entries(spec.exports).map(([name, claim]) => ({
      id: `${spec.name}#${name}`, package: spec.package, version: spec.version, export: name, rule: claim.rule,
      misuse: readFileSync(join(spec.directory, `${name}.misuse.tsx`), "utf8"),
      correct: readFileSync(join(spec.directory, `${name}.correct.tsx`), "utf8")
    }));
    const scratch = mkdtempSync(join(tmpdir(), "solid-checker-authored-"));
    writeFileSync(join(scratch, "cases.json"), json({ cases }));
    // The ledger writes each case beside the install, whose node_modules (an
    // ancestor) supplies the package and its Solid runtime.
    const run = spawnSync(process.execPath, [LEDGER, join(scratch, "out.json"), browser, "--cases", join(scratch, "cases.json"), "--concurrency", "2"],
      { env: { ...process.env, MISUSE_CASES_ROOT: join(identity.install, ".solid-checker-authored-probes") }, stdio: ["ignore", "inherit", "inherit"] });
    assert.equal(run.status, 0, `${spec.name}: the probe ledger failed`);
    for (const row of read(join(scratch, "out.json")).results) {
      results.push({ spec: spec.name, package: spec.package, version: spec.version, export: row.export,
        solidRuntime: identity.runtime, artifacts: identity.artifacts, rule: row.rule,
        verdict: row.runtime === "detected" ? "passed" : row.runtime,
        misuse: (row.misuse.diagnostics ?? []).map(({ code, site }) => ({ code, site })),
        correct: (row.correct.diagnostics ?? []).map(({ code, site }) => ({ code, site })) });
    }
  }
  results.sort((a, b) => `${a.spec}#${a.export}`.localeCompare(`${b.spec}#${b.export}`));
  writeFileSync(RESULTS, json({ format: 1, results }));
  for (const row of results) console.log(`${row.spec}#${row.export}: ${row.verdict}`);
}

/** Whether the claim for `name` has a passing pair on the spec's runtime. */
function passed(spec, name) {
  const results = existsSync(RESULTS) ? read(RESULTS).results : [];
  return results.some(row => row.spec === spec.name && row.export === name && row.verdict === "passed"
    && JSON.stringify(row.solidRuntime) === JSON.stringify(spec.solidRuntime.map(({ name, version }) => ({ name, version }))));
}

/** One authored document from one certified case document. */
function author(spec, bundle) {
  const document = read(join(ACCEPTED, bundle.document));
  const summaries = {};
  const shipped = [];
  for (const entrypoint of Object.values(document.entrypoints))
    for (const artifactCase of entrypoint.cases ?? [entrypoint]) {
      for (const field of Object.keys(artifactCase))
        assert(CASE_FIELDS.has(field), `${bundle.document}: case field ${field} would ship unprobed`);
      for (const [name, reference] of Object.entries(artifactCase.exports)) {
        const stability = typeof reference === "string" ? undefined : reference.stability;
        const { shape } = document.summaries[typeof reference === "string" ? reference : reference.summary];
        const claim = spec.exports[name];
        let id = `open-${shape}`;
        if (claim && passed(spec, name)) {
          id = `authored-${name}`;
          summaries[id] = { call: claim.call, shape };
          shipped.push(name);
        } else summaries[id] = { call: {}, shape };
        artifactCase.exports[name] = stability ? { stability, summary: id } : id;
      }
    }
  document.summaries = summaries;
  return { document, shipped };
}

function build() {
  const entries = [];
  const objects = new Map();
  const report = [];
  for (const spec of specs) {
    const integrity = Object.fromEntries(spec.solidRuntime.map(entry => [entry.name, entry]));
    assert.deepEqual(Object.keys(integrity).sort(), ["@solidjs/signals", "@solidjs/web", "solid-js"], `${spec.name}: solidRuntime`);
    for (const bundle of certifiedCases(spec)) {
      const receipt = read(join(ACCEPTED, bundle.receipt));
      const snapshotRoot = receipt.payload?.snapshotRoot;
      assert(snapshotRoot, `${bundle.receipt}: no snapshotRoot`);
      const { document, shipped } = author(spec, bundle);
      if (shipped.length === 0) continue;
      const bytes = Buffer.from(json(document));
      const member = `objects/${sha256(bytes)}.json`;
      objects.set(member, bytes);
      entries.push({
        packageName: bundle.packageName,
        specifier: bundle.specifier,
        packageVersion: bundle.packageVersion,
        packageIntegrity: bundle.packageIntegrity,
        requestedEntrypoint: bundle.requestedEntrypoint,
        exportConditions: bundle.exportConditions,
        runtimeTarget: bundle.runtimeTarget,
        declarationTarget: bundle.declarationTarget,
        snapshotRoot,
        solidRuntime: spec.solidRuntime,
        document: member,
        documentDigest: `sha256:${sha256(bytes)}`
      });
      report.push(`${bundle.packageName}@${bundle.packageVersion} [${bundle.exportConditions}] ${bundle.runtimeTarget}: ${shipped.join(", ")}`);
    }
  }
  const order = entry => JSON.stringify([entry.packageName, entry.packageVersion, entry.exportConditions, entry.runtimeTarget]);
  entries.sort((a, b) => order(a).localeCompare(order(b)));
  const embedded = `//! Generated by \`bun scripts/author-contracts.mjs build\`. Do not edit.
//!
//! The authored contract index and the bytes of every document it names
//! (ADR 0198).

pub(super) const INDEX: &[u8] = include_bytes!("../../../../../pkg/contracts/authored/index.json");

pub(super) const OBJECTS: &[(&str, &[u8])] = &[${[...objects.keys()].sort().map(member => `
    (
        "${member}",
        include_bytes!(
            "../../../../../pkg/contracts/authored/${member}"
        ),
    ),`).join("")}${objects.size ? "\n" : ""}];
`;
  return { index: json({ format: 1, entries }), embedded, objects, report };
}

if (command === "probe") probe(rest.find((arg, index) => !arg.startsWith("--") && !rest[index - 1]?.startsWith("--")));
else if (command === "build" || command === "check") {
  const { index, embedded, objects, report } = build();
  if (command === "check") {
    assert.equal(readFileSync(join(TIER, "index.json"), "utf8"), index, "pkg/contracts/authored/index.json is stale: run build");
    assert.equal(readFileSync(EMBEDDED, "utf8"), embedded, "authored_contracts/embedded.rs is stale: run build");
    const written = existsSync(join(TIER, "objects")) ? readdirSync(join(TIER, "objects")).map(name => `objects/${name}`).sort() : [];
    assert.deepEqual(written, [...objects.keys()].sort(), "pkg/contracts/authored/objects holds other documents: run build");
    for (const [member, bytes] of objects) assert.deepEqual(readFileSync(join(TIER, member)), bytes, `${member} is stale: run build`);
    console.log(`the authored tier is current: ${report.length} entries`);
  } else {
    mkdirSync(join(TIER, "objects"), { recursive: true });
    for (const name of readdirSync(join(TIER, "objects")))
      if (!objects.has(`objects/${name}`)) throw new Error(`objects/${name} is no longer written; remove it by hand after review`);
    for (const [member, bytes] of objects) writeFileSync(join(TIER, member), bytes);
    writeFileSync(join(TIER, "index.json"), index);
    writeFileSync(EMBEDDED, embedded);
    console.log(`wrote ${report.length} authored entries`);
    for (const line of report) console.log(`  ${line}`);
  }
} else {
  console.error("usage: author-contracts.mjs probe <chromium> --only <spec> --install <dir> | build | check");
  process.exit(2);
}
