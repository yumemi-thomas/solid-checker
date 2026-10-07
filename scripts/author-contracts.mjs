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
//   bun scripts/author-contracts.mjs identity --spec <spec> --install <dir> --proposal <file>
//       write the spec's identity.json from a `contract generate --host browser`
//       proposal for a version the certified tier does not carry (ADR 0207)
//
// A spec named `<package>@<version>+<label>` with `patchedInstall` in its
// spec.json is about one patched install (ADR 0208): its identity is that
// install's files, its probes run only where that exact patch is applied, and
// its entries are admitted only where the installed files reproduce them.
//
// A spec is a directory `pkg/contracts/authored/specs/<package>@<version>/`
// holding `spec.json` and, per claimed export, `<export>.misuse.tsx` and
// `<export>.correct.tsx` (or naming a shared directory of them in `pairs`). `spec.json` names the package version, the Solid
// runtime its claims are probed on, and per export the authored `call` and the
// misuse rule its pair exercises. A `call` that states more than one claim
// (callbacks and returns) names each further pair in `probes`, as `{ label,
// rule, why, scenario? }` exercised by `<export>.<label>.misuse.tsx` and
// `<export>.<label>.correct.tsx`; the call ships only when every pair passed.
//
// Each artifact case of that version starts from the certified document the
// compiled-in tier already carries for it, or, for a version it does not carry,
// from the spec's `identity.json` (ADR 0207): its identity, case structure and
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
import { propertyGetProbeDigest, validatePropertyGets } from "./lib/property-get-contracts.mjs";

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

/**
 * ADR 0226: an authored contract may close a domain only with a citation of
 * the installed source showing that export's complete behavior in it. Each
 * domain in `call.closed` needs a `closures[domain]` entry naming a file and
 * lines; a citation without a closed domain is refused too, so the two cannot
 * drift apart. `closures` is spec metadata and never enters the document.
 */
/** The claim domains a call may list operations in (schema `$defs/call`). */
const CLAIM_DOMAINS = ["callbacks", "reads", "writes", "creates", "invalidates", "throws", "returns", "cleanups", "disposals", "computations"];

function validateClosures(where, claim) {
  const closed = claim.call?.closed ?? [];
  const closures = claim.closures ?? {};
  for (const domain of closed) {
    const citation = closures[domain];
    assert(typeof citation === "string" && /\S+:\d+/.test(citation),
      `${where}: closed domain ${domain} needs a closures citation of the installed source (file:line)`);
  }
  for (const domain of Object.keys(closures))
    assert(closed.includes(domain), `${where}: closures cites ${domain}, which the call does not close`);
  // The decoder refuses an empty list for a domain left open
  // (contract_document.rs), and one undecodable authored document fails
  // every project. Refuse it here, before it ships.
  for (const domain of CLAIM_DOMAINS) {
    const items = claim.call?.[domain];
    if (Array.isArray(items) && items.length === 0)
      assert(closed.includes(domain), `${where}: open domain ${domain} has an empty list; omit it or close it`);
  }
}

// A directory whose name starts with `_` holds probe pairs that several specs
// share (`"pairs"` in spec.json, relative to the spec); it is not a spec.
const specs = readdirSync(join(TIER, "specs")).filter(name => !name.startsWith("_")).sort().map(name => {
  const directory = join(TIER, "specs", name);
  const spec = read(join(directory, "spec.json"));
  for (const [name, claim] of Object.entries(spec.exports)) {
    validatePropertyGets(claim.call);
    validateClosures(`${spec.package}@${spec.version}#${name}`, claim);
  }
  // ADR 0208: a spec about one patched install is named for its patch.
  const base = `${spec.package.replace("/", "+")}@${spec.version}`;
  assert.equal(name, spec.patchedInstall ? `${base}+${spec.patchedInstall.label}` : base, `${name}: directory names another version`);
  return { ...spec, name, directory, pairs: join(directory, spec.pairs ?? ".") };
});
const accepted = read(join(ACCEPTED, "index.json"));

/**
 * The certified cases of one version: one per (entrypoint, conditions, target).
 * Only browser cases: the probes run in Chrome, so a claim is evidence for the
 * browser host and nothing else.
 */
function certifiedCases(spec) {
  if (spec.identity) {
    const identity = read(join(spec.directory, spec.identity));
    assert.equal(identity.format, 1, `${spec.name}: identity format`);
    return identity.cases.map(entry => ({ ...entry, packageName: spec.package, packageVersion: spec.version }));
  }
  const cases = new Map();
  for (const bundle of accepted.bundles) {
    if (bundle.packageName !== spec.package || bundle.packageVersion !== spec.version) continue;
    if (!bundle.exportConditions.includes("browser")) continue;
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
/**
 * The patch files the tree above `install` records for `name@version`: Bun's
 * and pnpm's `patchedDependencies` (lockfile or package.json) and
 * `patch-package` files. A probe guard, not admission: admission reads every
 * mechanism itself (installed_patches.rs).
 */
/**
 * The `patchedDependencies` block of a lockfile, workspace or manifest: a JSON
 * object in `bun.lock`/`package.json`, an indented YAML map in pnpm files.
 * A lockfile names every installed package elsewhere, so only this block says
 * which ones are patched. Empty when there is none.
 */
function patchedDependenciesBlock(text) {
  const at = text.indexOf("patchedDependencies");
  if (at < 0) return "";
  const rest = text.slice(at);
  const lineEnd = rest.indexOf("\n");
  const brace = rest.indexOf("{");
  if (brace >= 0 && (lineEnd < 0 || brace < lineEnd)) {
    let depth = 0;
    for (let index = brace; index < rest.length; index += 1) {
      if (rest[index] === "{") depth += 1;
      else if (rest[index] === "}" && --depth === 0) return rest.slice(0, index + 1);
    }
    return rest;
  }
  const next = lineEnd < 0 ? -1 : rest.slice(lineEnd + 1).search(/^\S/m);
  return next < 0 ? rest : rest.slice(0, lineEnd + 1 + next);
}

function recordedPatches(install, name, version) {
  const key = `${name}@${version}`;
  const found = [];
  for (let directory = install; ; directory = dirname(directory)) {
    for (const file of ["bun.lock", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml"]) {
      const path = join(directory, file);
      if (!existsSync(path)) continue;
      const text = readFileSync(path, "utf8");
      const match = text.match(new RegExp(`"?${key.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&")}"?\\s*:\\s*"([^"]+\\.patch)"`));
      if (match) found.push(join(directory, match[1]));
      else if (patchedDependenciesBlock(text).includes(key)) found.push(path);
    }
    const patches = join(directory, "patches");
    if (existsSync(patches))
      for (const file of readdirSync(patches))
        if (file.startsWith(name.replace("/", "+")) || file.startsWith(encodeURIComponent(name)) || file.startsWith(name.replace("/", "%2F")))
          if (file.includes(version)) found.push(join(patches, file));
    if (directory === dirname(directory)) break;
  }
  return [...new Set(found)];
}

function probeInstall(spec, install) {
  const packageDirectory = installed(spec.package, install);
  const manifest = read(join(packageDirectory, "package.json"));
  assert.equal(manifest.version, spec.version, `${spec.name}: the probe install holds ${manifest.version}`);
  // ADR 0208: a spec about the published bytes is never probed on a patched
  // copy, and a spec about one patch only on an install holding that patch.
  const patches = recordedPatches(install, spec.package, spec.version);
  if (spec.patchedInstall) {
    assert(patches.some(path => existsSync(path) && `sha256:${sha256(readFileSync(path))}` === spec.patchedInstall.sha256),
      `${spec.name}: the probe install does not apply the stated patch`);
  } else {
    assert.deepEqual(patches, [], `${spec.name}: the probe install patches ${spec.package}@${spec.version}`);
  }
  if (spec.identity) {
    const root = snapshotRoot(packageDirectory, spec.package, spec.version);
    for (const bundle of certifiedCases(spec))
      assert.equal(root, bundle.snapshotRoot, `${spec.name}: the probe install's files are not the identity's snapshot`);
  }
  const artifacts = certifiedCases(spec).map(bundle => {
    const document = caseDocument(bundle);
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
    const cases = Object.entries(spec.exports).flatMap(([name, claim]) => pairsOf(name, claim).map(pair => ({
      id: `${spec.name}#${name}${pair.label ? `/${pair.label}` : ""}`, package: spec.package, version: spec.version,
      export: name, rule: pair.rule, ...(pair.label ? { label: pair.label } : {}),
      ...(pair.scenario ? { scenario: pair.scenario } : {}),
      misuse: readFileSync(join(spec.pairs, `${pair.file}.misuse.tsx`), "utf8"),
      correct: readFileSync(join(spec.pairs, `${pair.file}.correct.tsx`), "utf8")
    })));
    const scratch = mkdtempSync(join(tmpdir(), "solid-checker-authored-"));
    // Bind the bytes handed to the ledger before execution. Re-reading a
    // changed pair after a run must never stamp old observations as new input.
    const probeDigests = new Map(cases.map(entry => {
      const pair = pairsOf(entry.export, spec.exports[entry.export]).find(pair => pair.label === entry.label);
      return [entry.id, propertyGetProbeDigest(spec, entry.export, pair,
        entry.misuse, entry.correct, certifiedCases(spec))];
    }));
    writeFileSync(join(scratch, "cases.json"), json({ cases }));
    // The ledger writes each case beside the install, whose node_modules (an
    // ancestor) supplies the package and its Solid runtime.
    const run = spawnSync(process.execPath, [LEDGER, join(scratch, "out.json"), browser, "--cases", join(scratch, "cases.json"), "--concurrency", "2"],
      { env: { ...process.env, MISUSE_CASES_ROOT: join(identity.install, ".solid-checker-authored-probes") }, stdio: ["ignore", "inherit", "inherit"] });
    assert.equal(run.status, 0, `${spec.name}: the probe ledger failed`);
    const labels = new Map(cases.map(entry => [entry.id, entry.label]));
    for (const row of read(join(scratch, "out.json")).results) {
      const label = labels.get(row.id);
      const probeDigest = probeDigests.get(row.id);
      results.push({ spec: spec.name, package: spec.package, version: spec.version, export: row.export,
        ...(label ? { label } : {}),
        ...(probeDigest ? { propertyGetProbeDigest: probeDigest } : {}),
        solidRuntime: identity.runtime, artifacts: identity.artifacts, rule: row.rule,
        verdict: row.runtime === "detected" ? "passed" : row.runtime,
        misuse: (row.misuse.diagnostics ?? []).map(({ code, site }) => ({ code, site })),
        correct: (row.correct.diagnostics ?? []).map(({ code, site }) => ({ code, site })) });
    }
  }
  const key = row => `${row.spec}#${row.export}${row.label ? `/${row.label}` : ""}`;
  results.sort((a, b) => key(a).localeCompare(key(b)));
  writeFileSync(RESULTS, json({ format: 1, results }));
  for (const row of results) console.log(`${key(row)}: ${row.verdict}`);
}

/** The probe pairs of one export's claim: its own, then each in `probes`. */
/** A value shape with no reactive part: what a pure function can return. */
function plainOutput(shape) {
  if (!shape) return false;
  if (["undefined", "null", "plain"].includes(shape.kind)) return true;
  if (shape.kind === "array") return plainOutput(shape.element);
  return false;
}

function pairsOf(name, claim) {
  // ADR 0226: a pure function (`noop`, `clamp`, `keys`) has nothing a probe
  // can show; it is closures only, each cited. It names no rule and needs no
  // pair. Its operations may only be a bare return of a non-reactive value,
  // or a possible (min 0) ambient coercion of the caller's value, which can
  // yield an obligation and never a violation. Any other operation needs its
  // pair.
  if (claim.rule === undefined) {
    const unprobeable = operation => (operation.kind === "return" && plainOutput(operation.output)
        && operation.tracking === "untracked" && !operation.owner && !operation.guard)
      || (operation.kind === "invoke" && operation.protocol === "coerce" && (operation.count?.min ?? 0) === 0
        && operation.tracking === "ambient-at-execution" && operation.owner?.source === "ambient-at-execution");
    for (const operation of claim.call?.operations ?? [])
      assert(unprobeable(operation), `${name}: operation ${operation.id} needs a probe pair and its rule`);
    assert((claim.call?.closed ?? []).length > 0, `${name}: a claim with no rule must close a domain`);
    assert(!(claim.probes ?? []).length, `${name}: a closure-only claim has no probes`);
    return [];
  }
  return [{ label: undefined, rule: claim.rule, scenario: claim.scenario, file: name },
    ...(claim.probes ?? []).map(probe => {
      assert(probe.label && probe.rule && probe.why, `${name}: a probe needs a label, a rule and a why`);
      return { label: probe.label, rule: probe.rule, scenario: probe.scenario, file: `${name}.${probe.label}` };
    })];
}

/** Whether every pair of the claim for `name` passed on the spec's runtime. */
function passed(spec, name) {
  const results = existsSync(RESULTS) ? read(RESULTS).results : [];
  const runtime = JSON.stringify(spec.solidRuntime.map(({ name, version }) => ({ name, version })));
  return pairsOf(name, spec.exports[name]).every(pair => {
    const probeDigest = propertyGetProbeDigest(spec, name, pair,
      readFileSync(join(spec.pairs, `${pair.file}.misuse.tsx`), "utf8"),
      readFileSync(join(spec.pairs, `${pair.file}.correct.tsx`), "utf8"), certifiedCases(spec));
    return results.some(row => row.spec === spec.name && row.export === name
      && (row.label ?? undefined) === pair.label && row.verdict === "passed"
      && JSON.stringify(row.solidRuntime) === runtime
      && (!probeDigest || row.propertyGetProbeDigest === probeDigest));
  });
}

/** One authored document from one certified case document. */
/** The identity document of one case: certified, or from the spec's identity. */
function caseDocument(bundle) {
  return bundle.identityDocument ? structuredClone(bundle.identityDocument) : read(join(ACCEPTED, bundle.document));
}

/**
 * The snapshot root of a package as installed at `directory`, exactly as
 * `installed_package_snapshot_root` computes it: every regular file outside the
 * package's own top-level node_modules, with every directory those paths imply.
 */
function snapshotRoot(directory, name, version) {
  const files = [];
  const walk = (at, prefix) => {
    for (const entry of readdirSync(at, { withFileTypes: true })) {
      const path = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) { if (path !== "node_modules") walk(join(at, entry.name), path); }
      else if (entry.isFile()) files.push(path);
      else throw new Error(`${path} is not a regular file`);
    }
  };
  walk(directory, "");
  const order = (a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b));
  files.sort(order);
  const directories = new Set();
  for (const path of files) {
    const parts = path.split("/");
    for (let index = 1; index < parts.length; index += 1) directories.add(parts.slice(0, index).join("/"));
  }
  const hash = createHash("sha256");
  const field = bytes => { const length = Buffer.alloc(8); length.writeBigUInt64BE(BigInt(bytes.length)); hash.update(length); hash.update(bytes); };
  hash.update(Buffer.from("solid-checker:artifact-snapshot:v1\0", "latin1"));
  field(Buffer.from(name)); field(Buffer.from(version));
  for (const path of [...directories].sort(order)) { field(Buffer.from("directory")); field(Buffer.from(path)); }
  for (const path of files) { field(Buffer.from("file")); field(Buffer.from(path)); field(readFileSync(join(directory, path))); }
  return `sha256:${hash.digest("hex")}`;
}

/**
 * ADR 0207: a spec's identity.json from a `contract generate --host browser`
 * proposal. Each artifact case becomes one single-case document holding only
 * the identity fields; the proposal's claims are dropped, its summaries kept
 * only for their shape. The condition set and targets are the case's own, as
 * the proposal's certification inputs resolved them.
 */
function identity() {
  const only = option("--spec"), install = option("--install"), proposalPath = option("--proposal");
  assert(only && install && proposalPath, "usage: author-contracts.mjs identity --spec <spec> --install <dir> --proposal <file>");
  const spec = specs.find(candidate => candidate.name === only);
  assert(spec, `no spec ${only}`);
  const proposal = read(proposalPath);
  const inputs = read(`${proposalPath}.certification-inputs.json`);
  assert.equal(proposal.package.name, spec.package); assert.equal(proposal.package.version, spec.version);
  const packageDirectory = installed(spec.package, realpathSync(install));
  const root = snapshotRoot(packageDirectory, spec.package, spec.version);
  const cases = [];
  for (const [entrypoint, value] of Object.entries(proposal.entrypoints))
    for (const artifactCase of value.cases ?? [value]) {
      const runtimeTarget = artifactCase.artifact.path.replace(/^\.\//, "");
      const input = inputs.certificationInputs.find(row => row.entrypoint === entrypoint && row.resolution.runtime.path === join(packageDirectory, runtimeTarget));
      assert(input, `${spec.name}: no certification input resolves ${runtimeTarget}`);
      const summaries = {};
      for (const reference of Object.values(artifactCase.exports)) {
        const id = typeof reference === "string" ? reference : reference.summary;
        summaries[id] = { call: {}, shape: proposal.summaries[id].shape };
      }
      cases.push({
        specifier: input.resolution.specifier,
        requestedEntrypoint: entrypoint,
        // An ESM importer's resolution, as the corpus apps' certified bundles state it.
        exportConditions: [...new Set([...input.conditions, "import"])].sort(),
        runtimeTarget,
        declarationTarget: artifactCase.declarations.path.replace(/^\.\//, ""),
        packageIntegrity: proposal.package.integrity,
        snapshotRoot: root,
        identityDocument: { ...proposal, entrypoints: { [entrypoint]: { cases: [artifactCase] } }, summaries }
      });
    }
  writeFileSync(join(spec.directory, spec.identity ?? "identity.json"), json({ format: 1, cases }));
  console.log(`${spec.name}: wrote ${cases.length} identity cases`);
}

function author(spec, bundle) {
  const document = caseDocument(bundle);
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
      const snapshotRoot = bundle.snapshotRoot ?? read(join(ACCEPTED, bundle.receipt)).payload?.snapshotRoot;
      assert(snapshotRoot, `${spec.name}: no snapshotRoot for ${bundle.runtimeTarget}`);
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
        ...(spec.patchedInstall ? { patchedInstall: true } : {}),
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

if (command === "identity") identity();
else if (command === "probe") probe(rest.find((arg, index) => !arg.startsWith("--") && !rest[index - 1]?.startsWith("--")));
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
  console.error("usage: author-contracts.mjs probe <chromium> --only <spec> --install <dir> | identity --spec <spec> --install <dir> --proposal <file> | build | check");
  process.exit(2);
}
