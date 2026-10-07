#!/usr/bin/env bun

// The primary package-contract metric: of the places where real Solid 2
// applications import a third-party package export, how many does the checker
// certify, and what blocks the rest? (Owner decision 2026-09-28: success is
// certifying what applications actually import, weighted by the call sites
// that import it. `make certification-metric` -- every export of the top
// packages, equal weight -- stays as the secondary number.)
//
//   bun scripts/app-import-metric.mjs --pin <draft.json> [--work <dir>] [--corpus <out>]
//        network: clone each draft app at its branch head and write the pinned corpus
//   bun scripts/app-import-metric.mjs --run [--corpus <file>] [--work <dir>] [--only <id,...>]
//        [--checker <bin>] [--typefacts <bin>] [--package-metric <metric.json>]
//        [--jobs <n>] [--json <out>] [--markdown <out>] [--clean]
//        network: fetch each pinned commit, install it frozen, run the checker, measure
//   bun scripts/app-import-metric.mjs --measure [--corpus <file>] [--work <dir>] [--package-metric <f>]
//        [--json <out>] [--markdown <out>]
//        recompute the measurement from the extracted sites a previous --run saved
//
// `make app-import-metric` builds the release checker and runs `--run`.
//
// # The corpus
//
// `scripts/ecosystem-benchmark/app-import-corpus.json`: public Solid 2
// applications -- not libraries, forks or templates -- each pinned to a commit
// and a lockfile digest, with the tsconfig(s) that cover the application's own
// source and a sentence saying why it qualifies. `excluded` records the
// candidates that were reviewed and dropped, and why.
//
// # One run
//
// Every app is fetched at its pinned commit into `<work>/apps/<id>` and
// installed with its own package manager, frozen to its lockfile, scripts off.
// The install is reused while the lockfile digest, the package-manager command
// and the commit are unchanged (`<work>/apps/<id>.install.json`). An app whose
// lockfile digest differs from the pin, whose install fails, or whose project
// does not resolve a 2.x `solid-js` is *refused* and measured as such.
//
// Each project is analysed twice by the release checker, one-shot
// (`SOLID_CHECKER_DAEMON=0`, as the 2026-09-27 consumer-effect report did):
// once as shipped, and once with `--no-bundled-contracts`; then
// `--check-contracts` is asked for its per-package report, whose sentence for
// a compiled-in contract that was not admitted names the admission step that
// refused it.
//
// # One import site
//
// The unit is the checker's own: an **import site** is a location at which the
// acceptance gate raises `SC9005` for a package export when no contract is
// available -- one per value binding of an import declaration, and one per
// member access of a namespace import. A binding used only in types raises
// nothing, and a package whose manifest does not use Solid
// (`manifest_uses_solid`) needs no contract and raises nothing, so neither is
// a site. The denominator is therefore read from the tier-off run, where every
// site of every package the checker demands a contract for stops at the
// acceptance gate. Sites are attributed to `(file, module, export)` by the
// method of phase21's consumer-demand script: a collapsed finding's related
// locations are sites, and the export is read from the bytes each names.
//
// Each tier-off site is then classified by what the shipped run says about the
// same `(file, module, export)`:
//
//   certified     no finding: an accepted contract applies (the package is
//                 `accepted` in the shipped run's packageSummaries) and every
//                 domain the site needs is closed
//   open          an accepted contract applies and leaves a needed domain
//                 open (`unknown-contract-claims:<domains>`), omits the export,
//                 has no branch for the runtime environment, or leaves the
//                 execution of a callback passed at a call in the same file
//                 unknown
//   no contract   the shipped run still stops it at the acceptance gate
//   refused       an accepted contract whose claims cannot be used here
//                 (obsolete proof policy 1, or a claim the call cannot bind)
//   unexplained   no finding, and no accepted contract for the package: a
//                 disagreement between the two runs, never counted certified
//
// `solid-js`, `@solidjs/signals` and `@solidjs/web` are the dialect's
// vocabulary (ADR 0027) and never sites. A package the app links from its own
// workspace is first-party, and a site in a test file or a build config
// (`roleOfPath`) is not the application's runtime source; both are reported
// apart from the third-party headline.
//
// # Walls
//
// Every non-certified site gets its blocking causes. A no-contract site is
// explained by the checker's admission sentence when it gave one (an
// unreadable lockfile integrity, another installed version, a dependency
// environment entry -- the Solid runtime's or another's), else against the
// compiled-in authored tier (`pkg/contracts/authored/index.json`; the
// certified tier this once read is retired, ADR 0228): no entry for the
// package at all, none at the installed version, none for the imported
// specifier, or an entry whose environment the install does not match (with the checker's own admission sentence when packageSummaries
// carries one). An open site gets one cause per open domain, joined to the
// package side when `--package-metric` names a `make certification-metric`
// output that measured the same package version: that export's own cause for
// the domain, by the certification-metric classifier (`causeOf`). Walls are
// ranked by the sites they block; a site is *solely* blocked by a wall when it
// is that site's only cause.

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";

const root = resolve(import.meta.dirname, "..");
export const CORPUS_PATH = join(root, "scripts/ecosystem-benchmark/app-import-corpus.json");
const TIER_INDEX_PATH = join(root, "pkg/contracts/authored/index.json");
const DEFAULT_WORK = join(root, "rust/target/app-import-metric");

/// The runtime foundation (ADR 0027): dialect vocabulary, never a site.
export const DIALECT_OWNED = ["solid-js", "@solidjs/signals", "@solidjs/web"];

/// The acceptance gate's analysis context (`UNACCEPTED_IMPORT_CONTEXT`).
export const ACCEPTANCE_GATE = "no receipt-accepted contract matches this exact import";

/// Consumer-side open-claim names (`contracts.rs`) to the package side's four
/// consumer domains (certification-metric, census).
export const DOMAIN_OF_CLAIM = {
  callbacks: "callbacks",
  reactiveReads: "reads",
  returns: "returns",
  asyncBehavior: "returns",
  ownerRequirements: "creates"
};

export const STATES = ["certified", "open", "no contract", "refused", "unexplained"];

function fail(message) {
  console.error(`app-import-metric: ${message}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Findings -> sites (pure)
// ---------------------------------------------------------------------------

const PER_EXPORT = [
  /the reactivity contract for (\S+) has no entrypoint\/export summary for (?:imported|re-exported) export (\S+?);/,
  /the reactivity contract for (\S+) leaves .+? unknown for (?:imported|re-exported) export (\S+?);/,
  /the discovered reactivity contract for (\S+) was authorized only by obsolete proof policy 1; its claims cannot be used for (?:imported|re-exported) export (\S+?)(?: \(\d+ import sites\))?$/,
  /the reactivity contract for (\S+) states .+? for (?:imported|re-exported) export (\S+?), but this call site/,
  /the reactivity contract for (\S+) has different certified behavior for conditional runtime targets at (?:imported|re-exported) export (\S+?);/
];
const COLLAPSED = /this project has no accepted reactivity contract for (\S+?);/;
// ADR 0224: open claims shared by several exports are one finding per
// package, listing the exports. Each site's bytes are the export's name at an
// import or call; a site whose text is not a listed name stays "?".
const OPEN_CLAIMS_GROUP = /the reactivity contract for (\S+) leaves .+? unknown for \d+ (?:imported|called) exports: (.+?); code whose proof/;
const CALLBACK_EXECUTION = /callback parameter \d+ \(.*?\) of (.+?):([^\s:]+) reaches a call whose execution timing is unknown/;
const ENVIRONMENT_DEPENDENT = /has different certified behavior for conditional runtime targets/;

/// The package a module specifier names: `@scope/name/sub` -> `@scope/name`.
export function packageOfModule(module) {
  const parts = String(module).split("/");
  return module.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}

/// Which gate an `SC9005` finding stopped at, from its analysis context (the
/// message alone cannot say: the no-summary sentence is emitted both at the
/// acceptance gate and for an accepted contract that omits the export).
export function gateOf(finding) {
  if (finding?.id !== "SC9005") return null;
  const context = finding.analysisContext ?? "";
  const message = finding.message ?? "";
  if (context === ACCEPTANCE_GATE) return "acceptance";
  if (context.startsWith("unknown-contract-claims:")) return "open claims";
  if (context.startsWith("unbound-contract-claims:")) return "unbound claims";
  if (context.startsWith("obsolete-policy1")) return "obsolete policy 1";
  if (CALLBACK_EXECUTION.test(message)) return "callback execution";
  if (ENVIRONMENT_DEPENDENT.test(message)) return "environment branch";
  if (PER_EXPORT[0].test(message)) return "export missing";
  return "unrecognized";
}

/// The open domains a finding names: `unknown-contract-claims:a,b` -> [a, b];
/// the other open gates name one pseudo-domain each.
export function openDomainsOf(finding) {
  const gate = gateOf(finding);
  if (gate === "open claims") return finding.analysisContext.split(":")[1].split(",").filter(Boolean);
  if (gate === "export missing") return ["export not summarized"];
  if (gate === "environment branch") return ["environment branch"];
  if (gate === "callback execution") return ["callbacks (execution)"];
  return [];
}

const IMPORT_DECLARATION = /^\s*import\s+(?!type\b)(.*?)\s+from\s+['"]/s;
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;

/// The imported names of an import declaration's value bindings, in order:
/// `import d, { a, b as c, type T } from "m"` -> ["default", "a", "b"]. A
/// namespace binding is not an export (its member accesses are the sites).
/// `null` when the text is not a declaration this reads.
export function valueBindings(declaration) {
  const match = IMPORT_DECLARATION.exec(declaration);
  if (!match) return null;
  const clause = match[1].trim();
  const names = [];
  const braces = /\{(.*)\}/s.exec(clause);
  const head = braces ? clause.slice(0, braces.index) : clause;
  for (const part of head.split(",").map(piece => piece.trim()).filter(Boolean)) {
    if (part.startsWith("*")) continue;
    if (!IDENTIFIER.test(part)) return null;
    names.push("default");
  }
  if (braces) {
    for (const part of braces[1].split(",").map(piece => piece.trim()).filter(Boolean)) {
      if (/^type\s/.test(part)) continue;
      const imported = part.split(/\s+as\s+/)[0].trim();
      if (!IDENTIFIER.test(imported)) return null;
      names.push(imported);
    }
  }
  return names;
}

function locationsOf(finding) {
  return [finding.primaryLocation ?? {}, ...(finding.relatedLocations ?? [])];
}

/// Every site of one `SC9005` finding, as `{ module, export, path }`
/// (`export` is `"?"` when the bytes do not settle it one-to-one), plus the
/// gate. `readSource(path)` returns the file's bytes (a Buffer) or null.
export function sitesOf(finding, readSource) {
  const gate = gateOf(finding);
  if (!gate) return { gate, sites: [] };
  const message = finding.message ?? "";
  const locations = locationsOf(finding);
  if (gate === "callback execution") {
    const match = CALLBACK_EXECUTION.exec(message);
    return { gate, sites: match ? [{ callee: match[1], export: match[2], path: locations[0].path }] : [] };
  }
  for (const pattern of PER_EXPORT) {
    const match = pattern.exec(message);
    if (match) return { gate, sites: locations.map(location => ({ module: match[1], export: match[2], path: location.path })) };
  }
  const group = OPEN_CLAIMS_GROUP.exec(message);
  if (group) {
    const listed = new Set(group[2].replace(/, and \d+ more$/, "").split(", "));
    return {
      gate,
      sites: locations.map(location => {
        const data = location.path ? readSource(location.path) : null;
        const text = data && location.startByte != null && location.endByte != null
          ? data.subarray(location.startByte, location.endByte).toString("utf8")
          : null;
        return { module: group[1], export: text && listed.has(text) ? text : "?", path: location.path };
      })
    };
  }
  const collapsed = COLLAPSED.exec(message);
  if (!collapsed) return { gate: "unrecognized", sites: locations.map(location => ({ module: "?", export: "?", path: location.path })) };
  const module = collapsed[1];
  const sites = [];
  const declarations = new Map();
  for (const location of locations) {
    const data = location.path ? readSource(location.path) : null;
    const { startByte: start, endByte: end } = location;
    if (!data || start == null || end == null) {
      sites.push({ module, export: "?", path: location.path });
      continue;
    }
    const text = data.subarray(start, end).toString("utf8");
    if (IDENTIFIER.test(text) && data.subarray(start - 1, start).toString() === ".") {
      sites.push({ module, export: text, path: location.path });
      continue;
    }
    const key = `${location.path}\u0000${start}\u0000${end}`;
    const held = declarations.get(key) ?? { path: location.path, text, repeats: 0 };
    held.repeats += 1;
    declarations.set(key, held);
  }
  for (const { path, text, repeats } of declarations.values()) {
    const names = valueBindings(text);
    if (names && names.length === repeats) for (const name of names) sites.push({ module, export: name, path });
    else for (let index = 0; index < repeats; index += 1) sites.push({ module, export: "?", path });
  }
  return { gate, sites };
}

const keyOf = (path, module, exportName) => `${path}\u0000${module}\u0000${exportName}`;

/// The module a callback-execution finding names: `pkg.` is the package root,
/// `pkg./sub` its subpath; `current project.` is the project's own code.
export function moduleOfCallee(callee) {
  if (!callee || callee.startsWith("current project")) return null;
  const match = /^(.+?)\.(\/.*)?$/.exec(callee);
  if (!match) return callee;
  return match[2] ? `${match[1]}${match[2]}` : match[1];
}

/// Classifies one project's import sites. `on` and `off` are the checker's
/// JSON outputs as shipped and with `--no-bundled-contracts`; `resolvePackage`
/// maps `(path, module)` to `{ package, version, origin }` (origin
/// `third-party`, `workspace` or `unresolved`). Returns one row per
/// `(path, module, export)` with its site count and state, plus what the two
/// runs disagree on.
export function classifyProject({ on, off, readSource, resolvePackage }) {
  const denominator = new Map();
  for (const finding of off.findings ?? []) {
    const { gate, sites } = sitesOf(finding, readSource);
    if (gate !== "acceptance") continue;
    for (const site of sites) {
      const key = keyOf(site.path, site.module, site.export);
      const held = denominator.get(key) ?? { path: site.path, module: site.module, export: site.export, sites: 0 };
      held.sites += 1;
      denominator.set(key, held);
    }
  }
  const shipped = new Map();
  const callbackCalls = new Map();
  let projectCallbackExecution = 0;
  const onOnly = [];
  for (const finding of on.findings ?? []) {
    const { gate, sites } = sitesOf(finding, readSource);
    if (!gate) continue;
    if (gate === "callback execution") {
      for (const site of sites) {
        const module = moduleOfCallee(site.callee);
        if (!module) {
          projectCallbackExecution += 1;
          continue;
        }
        const key = keyOf(site.path, module, site.export);
        callbackCalls.set(key, (callbackCalls.get(key) ?? 0) + 1);
      }
      continue;
    }
    for (const site of sites) {
      const key = keyOf(site.path, site.module, site.export);
      const held = shipped.get(key) ?? { gates: new Set(), domains: new Set(), sites: 0 };
      held.gates.add(gate);
      for (const domain of openDomainsOf(finding)) held.domains.add(domain);
      held.sites += 1;
      shipped.set(key, held);
    }
  }
  const accepted = new Set(
    (on.packageSummaries ?? []).filter(entry => entry.evidence === "accepted").map(entry => `${entry.name}@${entry.version}`)
  );
  const acceptedNames = new Set((on.packageSummaries ?? []).filter(entry => entry.evidence === "accepted").map(entry => entry.name));
  const refusals = {};
  for (const entry of on.packageSummaries ?? []) {
    if (entry.evidence !== "accepted" && entry.detail) (refusals[entry.name] ??= new Set()).add(entry.detail);
  }
  const rows = [];
  for (const [key, row] of denominator) {
    const name = packageOfModule(row.module);
    if (DIALECT_OWNED.includes(name)) continue;
    const identity = resolvePackage(row.path, row.module) ?? { package: name, version: null, origin: "unresolved" };
    const held = shipped.get(key);
    const callbacks = callbackCalls.get(key) ?? 0;
    let state;
    let domains = [];
    if (held?.gates.has("acceptance")) state = "no contract";
    else if (held && (held.gates.has("obsolete policy 1") || held.gates.has("unbound claims"))) state = "refused";
    else if (held?.domains.size || callbacks) {
      state = "open";
      domains = [...(held?.domains ?? []), ...(callbacks ? ["callbacks (execution)"] : [])];
    } else if (held) state = "unexplained";
    else if (acceptedNames.has(name)) state = "certified";
    else state = "unexplained";
    rows.push({
      ...row,
      package: identity.package ?? name,
      version: identity.version,
      origin: identity.origin,
      state,
      domains: [...new Set(domains)].sort(),
      gates: held ? [...held.gates].sort() : [],
      acceptedContract: accepted.has(`${identity.package ?? name}@${identity.version}`) || acceptedNames.has(name),
      admission: [...(refusals[name] ?? [])]
    });
  }
  for (const [key, held] of shipped) {
    if (denominator.has(key)) continue;
    const [path, module, exportName] = key.split("\u0000");
    if (DIALECT_OWNED.includes(packageOfModule(module))) continue;
    onOnly.push({ path, module, export: exportName, sites: held.sites, gates: [...held.gates].sort() });
  }
  rows.sort((left, right) => (left.path + left.module + left.export).localeCompare(right.path + right.module + right.export));
  return { rows, onOnly, projectCallbackExecution, accepted: [...accepted].sort() };
}

/// Merges an app's projects: a file two tsconfigs both analyse is counted
/// once, by the first project in the corpus entry that saw it.
export function mergeProjects(projects) {
  const seen = new Map();
  for (const project of projects) {
    for (const row of project.rows ?? []) {
      const key = keyOf(row.path, row.module, row.export);
      if (!seen.has(key)) seen.set(key, { ...row, project: project.project });
    }
  }
  return [...seen.values()];
}

// ---------------------------------------------------------------------------
// Walls (pure)
// ---------------------------------------------------------------------------

/// The entries of a tier index by package: `{ versions: Set, specifiers:
/// Map<version, Set> }`, from an authored index (`entries`) or a certified one
/// (`bundles`).
const tierEntries = index => [...(index?.entries ?? []), ...(index?.bundles ?? [])];

export function tierByPackage(index) {
  const tier = new Map();
  for (const bundle of tierEntries(index)) {
    const entry = tier.get(bundle.packageName) ?? { versions: new Set(), specifiers: new Map() };
    entry.versions.add(bundle.packageVersion);
    const specifiers = entry.specifiers.get(bundle.packageVersion) ?? new Set();
    specifiers.add(bundle.specifier);
    entry.specifiers.set(bundle.packageVersion, specifiers);
    tier.set(bundle.packageName, entry);
  }
  return tier;
}

const list = values => [...values].sort().join(", ");

/// The wall a checker admission sentence names ("a compiled-in contract
/// exists for this package and was not admitted: <reason>"), with versions and
/// digests kept in the key and the class generalised.
export function admissionCause(row, reason) {
  const text = String(reason);
  const pkg = `${row.package}@${row.version}`;
  if (/no exact lockfile integrity/.test(text)) {
    return { class: "no contract: lockfile integrity not read", key: row.lockfileShape ?? "the installed package has no exact lockfile integrity", detail: text };
  }
  const differs = /its dependency environment differs: (\S+)(?: resolved from \S+)? installed (\S+), certified (\S+?)[;,.]?(?:\s|$)/.exec(text);
  if (differs && DIALECT_OWNED.includes(differs[1])) {
    return {
      class: "no contract: no bundle on the installed Solid runtime",
      key: `${pkg} (installed ${differs[1]} ${differs[2]}; a bundle is certified on ${differs[3]})`,
      detail: text
    };
  }
  if (differs) {
    return { class: "no contract: environment not admitted", key: `${pkg}: ${differs[1]} installed ${differs[2]}, certified ${differs[3]}`, detail: text };
  }
  const version = /the installed package is (\S+), not the certified (\S+)/.exec(text);
  if (version) {
    return { class: "no contract: installed version not in the tier", key: `${row.package}@${version[1]} (tier certified ${version[2]})`, detail: text };
  }
  if (/patch/i.test(text)) return { class: "no contract: installed copy patched", key: pkg, detail: text };
  return { class: "no contract: environment not admitted", key: `${pkg}: ${text.replace(/sha(256|512)[-:][A-Za-z0-9+/=]+/g, "<digest>").slice(0, 140)}`, detail: text };
}

/// Why a no-contract site has no contract: the checker's own admission
/// sentence when it gave one, else the compiled-in tier's coverage.
export function noContractCause(row, tier) {
  if (row.admission?.length) return admissionCause(row, row.admission[0]);
  const entry = tier.get(row.package);
  if (!entry) return { class: "no contract: package not in the tier", key: row.package };
  if (!row.version || !entry.versions.has(row.version)) {
    return {
      class: "no contract: installed version not in the tier",
      key: `${row.package}@${row.version ?? "?"}`,
      detail: `tier has ${list(entry.versions)}`
    };
  }
  const specifiers = entry.specifiers.get(row.version);
  if (!specifiers.has(row.module)) {
    return {
      class: "no contract: specifier not in the tier",
      key: `${row.module} (${row.package}@${row.version})`,
      detail: `tier has ${list(specifiers)}`
    };
  }
  // The environment probe (`probeEnvironments`) names the first entry of the
  // bundle's recorded dependency environment the install resolves differently.
  const probe = row.environment ?? null;
  const mismatch = probe?.mismatch ?? null;
  return {
    class: "no contract: environment not admitted",
    key: mismatch ? `${row.package}@${row.version}: ${mismatch.name} ${mismatch.installed ?? "not installed"} (bundle: ${mismatch.bundle})` : `${row.package}@${row.version}`,
    detail: row.admission?.[0] ?? (probe ? probe.summary : "the bundle's recorded environment does not match this install")
  };
}

/// The first entry of a recorded dependency environment the install resolves
/// differently: `entries` are the bundle's `{ name, version, integrity }`,
/// `installed` maps a name to `{ version, integrity }` as Node resolution
/// from the installed package reaches it (absent when it reaches none).
export function environmentMismatch(entries, installed) {
  const order = entry => (DIALECT_OWNED.includes(entry.name) ? 0 : 1);
  for (const entry of [...entries].sort((left, right) => order(left) - order(right) || left.name.localeCompare(right.name))) {
    const found = installed[entry.name];
    if (!found) return { name: entry.name, bundle: entry.version, installed: null };
    if (found.version !== entry.version) return { name: entry.name, bundle: entry.version, installed: found.version };
    if (found.integrity && entry.integrity && found.integrity !== entry.integrity) {
      return { name: entry.name, bundle: `${entry.version} ${entry.integrity.slice(0, 16)}…`, installed: `${found.version} ${found.integrity.slice(0, 16)}…` };
    }
  }
  return null;
}

/// The certification-metric's entrypoint for a specifier: `pkg` -> `.`,
/// `pkg/sub` -> `./sub`.
export function entrypointOf(module) {
  const name = packageOfModule(module);
  return module === name ? "." : `.${module.slice(name.length)}`;
}

/// `metric.json` of `make certification-metric` -> `package@version` ->
/// `entrypoint\0export` -> export row (with `causes`).
export function packageMetricIndex(metric) {
  const index = new Map();
  for (const entry of metric?.packages ?? []) {
    const exports = new Map();
    for (const row of entry.exports ?? []) exports.set(`${row.entrypoint}\u0000${row.export}`, row);
    index.set(`${entry.package}@${entry.version}`, { exports, bucket: entry.counts, certifiable: entry.certifiable });
  }
  return index;
}

/// One cause per open domain of an open site; the package side's own cause
/// for the domain when the certification metric measured this version.
export function openCauses(row, packageIndex) {
  const measured = packageIndex?.get(`${row.package}@${row.version}`);
  return row.domains.map(claim => {
    const domain = DOMAIN_OF_CLAIM[claim] ?? claim;
    if (!DOMAIN_OF_CLAIM[claim]) return { class: `open: ${claim}`, key: `${row.package}@${row.version}` };
    if (!measured) {
      return { class: `open: ${domain}`, key: `${row.package}@${row.version} (not in the certification-metric corpus at this version)` };
    }
    const answer = measured.exports.get(`${entrypointOf(row.module)}\u0000${row.export}`);
    if (!answer) return { class: `open: ${domain}`, key: `${row.package}@${row.version}: export not in the measured surface` };
    const cause = (answer.causes ?? []).find(entry => entry.domain === domain);
    if (!cause) return { class: `open: ${domain}`, key: `${row.package}@${row.version}: closed on the package side (${answer.bucket}), open at this site` };
    return { class: `open: ${domain}`, key: `${cause.class}: ${cause.key}`, package: `${row.package}@${row.version}` };
  });
}

/// Every non-certified third-party site's causes.
export function causesOf(row, tier, packageIndex) {
  if (row.state === "no contract") return [noContractCause(row, tier)];
  if (row.state === "open") return openCauses(row, packageIndex);
  if (row.state === "refused") return [{ class: "refused", key: `${row.gates.join(", ")} (${row.package}@${row.version})` }];
  if (row.state === "unexplained") return [{ class: "unexplained", key: `${row.package}@${row.version}` }];
  return [];
}

/// Ranks walls by the sites they block. `rows` are third-party site rows with
/// `app` and `causes`.
export function rankWalls(rows) {
  const walls = new Map();
  for (const row of rows) {
    const causes = row.causes ?? [];
    const distinct = new Map(causes.map(cause => [`${cause.class}\u0000${cause.key}`, cause]));
    for (const [id, cause] of distinct) {
      const wall = walls.get(id) ?? { class: cause.class, key: cause.key, detail: cause.detail ?? null, sites: 0, sole: 0, apps: new Set(), packages: new Set(), exports: new Map() };
      wall.sites += row.sites;
      if (distinct.size === 1) wall.sole += row.sites;
      wall.apps.add(row.app);
      wall.packages.add(`${row.package}@${row.version}`);
      const exportKey = `${row.module}:${row.export}`;
      wall.exports.set(exportKey, (wall.exports.get(exportKey) ?? 0) + row.sites);
      walls.set(id, wall);
    }
  }
  return [...walls.values()]
    .map(wall => ({
      class: wall.class,
      key: wall.key,
      detail: wall.detail,
      sites: wall.sites,
      sole: wall.sole,
      apps: wall.apps.size,
      packages: [...wall.packages].sort(),
      topExports: [...wall.exports].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0])).slice(0, 5)
    }))
    .sort((left, right) => right.sites - left.sites || right.sole - left.sole || left.key.localeCompare(right.key));
}

/// Walls grouped by class (a site counts once per class).
export function rankClasses(rows) {
  const classes = new Map();
  for (const row of rows) {
    const distinct = new Set((row.causes ?? []).map(cause => cause.class));
    for (const group of distinct) {
      const entry = classes.get(group) ?? { class: group, sites: 0, sole: 0, apps: new Set() };
      entry.sites += row.sites;
      if (distinct.size === 1) entry.sole += row.sites;
      entry.apps.add(row.app);
      classes.set(group, entry);
    }
  }
  return [...classes.values()].map(entry => ({ ...entry, apps: entry.apps.size })).sort((left, right) => right.sites - left.sites);
}

// ---------------------------------------------------------------------------
// Aggregation (pure)
// ---------------------------------------------------------------------------

const ratio = (part, whole) => (whole === 0 ? null : part / whole);

function tally(rows) {
  const counts = Object.fromEntries(STATES.map(state => [state, 0]));
  for (const row of rows) counts[row.state] += row.sites;
  const total = Object.values(counts).reduce((sum, value) => sum + value, 0);
  return { ...counts, total, certifiedShare: ratio(counts.certified, total) };
}

/// What a site's file is: `test` (a test, story or test helper), `tooling`
/// (a build or tool config: `vite.config.ts` and its relatives), or `app`,
/// the application's own runtime source. The headline counts `app` sites only;
/// the other two are reported beside it.
export function roleOfPath(path) {
  const text = String(path ?? "");
  const base = text.split("/").pop();
  if (/(^|\/)(__tests__|__mocks__|tests?|e2e|spec|stories)\//i.test(text)) return "test";
  if (/\.(test|spec|stories|story|bench)\.[cm]?[jt]sx?$/i.test(base)) return "test";
  if (/^(test-?utils?|testing|setup-?tests?|vitest\.setup|test-setup)\.[cm]?[jt]sx?$/i.test(base)) return "test";
  if (/\.config\.[cm]?[jt]s$/i.test(base)) return "tooling";
  return "app";
}

/// The whole measurement from per-app extractions. `apps` are
/// `{ id, status, refusal?, projects: [{ project, rows, onOnly, ... }], ... }`.
export function measure({ apps, tierIndex, packageMetric = null }) {
  const tier = tierByPackage(tierIndex);
  const packageIndex = packageMetric ? packageMetricIndex(packageMetric) : null;
  const perApp = [];
  const thirdParty = [];
  const workspace = [];
  const tests = [];
  const tooling = [];
  for (const app of apps) {
    if (app.status !== "analysed") {
      perApp.push({ id: app.id, status: app.status, refusal: app.refusal ?? null, counts: null });
      continue;
    }
    const rows = mergeProjects(app.projects).map(row => ({ ...row, app: app.id, role: roleOfPath(row.path), lockfileShape: app.lockfileShape }));
    const own = rows.filter(row => row.origin === "workspace");
    const outside = rows.filter(row => row.origin !== "workspace" && row.role !== "app");
    const third = rows.filter(row => row.origin !== "workspace" && row.role === "app");
    for (const row of outside) (row.role === "test" ? tests : tooling).push(row);
    for (const row of third) row.causes = causesOf(row, tier, packageIndex);
    thirdParty.push(...third);
    workspace.push(...own);
    perApp.push({
      id: app.id,
      status: app.status,
      solid: app.solid ?? null,
      counts: tally(third),
      workspace: tally(own),
      tests: tally(outside.filter(row => row.role === "test")),
      tooling: tally(outside.filter(row => row.role === "tooling")),
      packages: [...new Set(third.map(row => `${row.package}@${row.version}`))].sort(),
      onOnly: app.projects.reduce((sum, project) => sum + (project.onOnly ?? []).reduce((total, entry) => total + entry.sites, 0), 0),
      unattributed: third.filter(row => row.export === "?").reduce((sum, row) => sum + row.sites, 0),
      projectCallbackExecution: app.projects.reduce((sum, project) => sum + (project.projectCallbackExecution ?? 0), 0),
      wallMs: app.wallMs ?? null
    });
  }
  const measured = perApp.filter(app => app.counts && app.counts.total > 0);
  const perPackage = new Map();
  for (const row of thirdParty) {
    const id = `${row.package}@${row.version}`;
    const entry = perPackage.get(id) ?? { package: row.package, version: row.version, rows: [], apps: new Set() };
    entry.rows.push(row);
    entry.apps.add(row.app);
    perPackage.set(id, entry);
  }
  const packages = [...perPackage.values()]
    .map(entry => ({
      package: entry.package,
      version: entry.version,
      apps: entry.apps.size,
      inTier: tier.has(entry.package),
      counts: tally(entry.rows),
      domains: Object.entries(
        entry.rows.filter(row => row.state === "open").reduce((acc, row) => {
          for (const domain of row.domains) acc[domain] = (acc[domain] ?? 0) + row.sites;
          return acc;
        }, {})
      ).sort((left, right) => right[1] - left[1])
    }))
    .sort((left, right) => right.counts.total - left.counts.total || left.package.localeCompare(right.package));
  const nonCertified = thirdParty.filter(row => row.state !== "certified");
  return {
    generatedAt: new Date().toISOString(),
    headline: {
      apps: apps.length,
      analysed: perApp.filter(app => app.status === "analysed").length,
      appsWithSites: measured.length,
      pooled: tally(thirdParty),
      meanPerApp: ratio(measured.reduce((sum, app) => sum + app.counts.certifiedShare, 0), measured.length),
      appsWithAnyCertified: measured.filter(app => app.counts.certified > 0).length,
      workspace: tally(workspace),
      tests: tally(tests),
      tooling: tally(tooling),
      testPackages: [...new Set(tests.map(row => row.package))].sort(),
      toolingPackages: [...new Set(tooling.map(row => row.package))].sort(),
      packages: packages.length
    },
    apps: perApp,
    packages,
    classes: rankClasses(nonCertified),
    walls: rankWalls(nonCertified),
    sites: thirdParty.map(({ causes, ...row }) => ({ ...row, causes }))
  };
}

// ---------------------------------------------------------------------------
// Rendering (pure)
// ---------------------------------------------------------------------------

const percent = value => (value == null ? "—" : `${(value * 100).toFixed(1)} %`);

export function renderMarkdown(result) {
  const lines = [];
  const { headline } = result;
  lines.push("# App import metric", "");
  lines.push(`Generated ${result.generatedAt}.`, "");
  lines.push("| | value |", "| --- | ---: |");
  lines.push(`| apps in the corpus / analysed / with third-party import sites | ${headline.apps} / ${headline.analysed} / ${headline.appsWithSites} |`);
  lines.push(`| third-party import sites | ${headline.pooled.total} |`);
  lines.push(`| **certified, pooled** | **${headline.pooled.certified} (${percent(headline.pooled.certifiedShare)})** |`);
  lines.push(`| certified, mean per app | ${percent(headline.meanPerApp)} |`);
  lines.push(`| open / no contract / refused / unexplained | ${headline.pooled.open} / ${headline.pooled["no contract"]} / ${headline.pooled.refused} / ${headline.pooled.unexplained} |`);
  lines.push(`| apps with at least one certified site | ${headline.appsWithAnyCertified} |`);
  lines.push(`| distinct third-party package versions | ${headline.packages} |`);
  lines.push(`| workspace (first-party) package sites, not in the headline | ${headline.workspace.total} |`);
  lines.push(`| sites in test files / build config, not in the headline | ${headline.tests.total} / ${headline.tooling.total} |`, "");
  lines.push("## Per app", "", "| app | status | sites | certified | open | no contract | refused | unexplained | share | workspace sites | wall |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
  for (const app of result.apps) {
    if (!app.counts) {
      lines.push(`| ${app.id} | ${app.status}: ${String(app.refusal ?? "").slice(0, 80)} | — | — | — | — | — | — | — | — | — |`);
      continue;
    }
    const c = app.counts;
    lines.push(`| ${app.id} | ${app.status} | ${c.total} | ${c.certified} | ${c.open} | ${c["no contract"]} | ${c.refused} | ${c.unexplained} | ${percent(c.certifiedShare)} | ${app.workspace.total} | ${app.wallMs == null ? "—" : `${(app.wallMs / 1000).toFixed(1)} s`} |`);
  }
  lines.push("", "## Per package", "", "| package | apps | sites | certified | open | no contract | refused | in tier | open domains (sites) |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |");
  for (const entry of result.packages) {
    const c = entry.counts;
    lines.push(`| \`${entry.package}@${entry.version}\` | ${entry.apps} | ${c.total} | ${c.certified} | ${c.open} | ${c["no contract"]} | ${c.refused} | ${entry.inTier ? "yes" : "no"} | ${entry.domains.map(([domain, sites]) => `${domain} ${sites}`).join(", ")} |`);
  }
  lines.push("", "## Walls by class", "", "| class | sites blocked | solely | apps |", "| --- | ---: | ---: | ---: |");
  for (const entry of result.classes) lines.push(`| ${entry.class} | ${entry.sites} | ${entry.sole} | ${entry.apps} |`);
  lines.push("", "## Walls", "", "| # | class | wall | sites | solely | apps | top exports |", "| ---: | --- | --- | ---: | ---: | ---: | --- |");
  result.walls.slice(0, 40).forEach((wall, index) => {
    lines.push(`| ${index + 1} | ${wall.class} | ${wall.key}${wall.detail && !wall.key.includes(String(wall.detail).slice(0, 40)) ? ` — ${String(wall.detail).slice(0, 160)}` : ""} | ${wall.sites} | ${wall.sole} | ${wall.apps} | ${wall.topExports.map(([name, sites]) => `${name} ${sites}`).join(", ")} |`);
  });
  return `${lines.join("\n")}\n`;
}

// ---------------------------------------------------------------------------
// Fetch, install, analyse (impure)
// ---------------------------------------------------------------------------

function sh(command, args, options = {}) {
  const started = Date.now();
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 30, ...options });
  return { ...result, wallMs: Date.now() - started, ok: result.status === 0 };
}

/// `sh` without blocking the event loop, so installs can run in parallel.
function shAsync(command, args, options = {}) {
  const started = Date.now();
  return new Promise(resolvePromise => {
    const child = spawn(command, args, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", chunk => (stdout += chunk));
    child.stderr.on("data", chunk => (stderr += chunk));
    child.on("error", error => resolvePromise({ status: null, stdout, stderr: `${stderr}${error.message}`, ok: false, wallMs: Date.now() - started }));
    child.on("close", status => resolvePromise({ status, stdout, stderr, ok: status === 0, wallMs: Date.now() - started }));
  });
}

function git(directory, ...args) {
  const result = sh("git", ["-C", directory, ...args]);
  if (!result.ok) throw new Error(`git ${args.join(" ")}: ${result.stderr.trim().slice(0, 400)}`);
  return result.stdout.trim();
}

const sha256 = path => `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
const LOCKFILES = ["pnpm-lock.yaml", "bun.lock", "bun.lockb", "package-lock.json", "yarn.lock"];

/// The lockfile governing an app directory: the nearest one at or above it,
/// inside the repository.
function findLockfile(repository, app) {
  let directory = resolve(repository, app);
  for (;;) {
    for (const name of LOCKFILES) if (existsSync(join(directory, name))) return relative(repository, join(directory, name));
    if (resolve(directory) === resolve(repository)) return null;
    directory = dirname(directory);
  }
}

/// The install command for a lockfile, from the nearest `packageManager`
/// field when there is one.
export function installCommand(lockfile, packageManager, lockfileText = "") {
  const name = lockfile.split("/").pop();
  const declared = packageManager ? String(packageManager).split("+")[0] : null;
  if (name === "pnpm-lock.yaml") {
    let spec = declared?.startsWith("pnpm@") ? declared : null;
    if (!spec) {
      const version = /lockfileVersion:\s*'?([\d.]+)/.exec(lockfileText)?.[1] ?? "9.0";
      spec = version.startsWith("9") ? "pnpm@10.33.2" : version.startsWith("6") ? "pnpm@8.15.9" : "pnpm@7.33.7";
    }
    return { manager: spec, command: "npx", args: ["-y", spec, "install", "--frozen-lockfile", "--ignore-scripts"] };
  }
  if (name === "bun.lock" || name === "bun.lockb") {
    return { manager: `bun@${Bun.version}`, command: "bun", args: ["install", "--frozen-lockfile", "--ignore-scripts"] };
  }
  if (name === "package-lock.json") return { manager: "npm", command: "npm", args: ["ci", "--ignore-scripts", "--no-audit", "--no-fund"] };
  if (name === "yarn.lock") {
    const spec = declared?.startsWith("yarn@") ? declared : "yarn@1.22.22";
    return spec.startsWith("yarn@1.")
      ? { manager: spec, command: "npx", args: ["-y", spec, "install", "--frozen-lockfile", "--ignore-scripts"] }
      : { manager: spec, command: "npx", args: ["-y", `@yarnpkg/cli-dist@${spec.slice(5)}`, "install", "--immutable", "--mode=skip-build"] };
  }
  return null;
}

function nearestPackageManager(repository, lockfile) {
  let directory = resolve(repository, dirname(lockfile));
  for (;;) {
    const manifest = join(directory, "package.json");
    if (existsSync(manifest)) {
      try {
        const declared = JSON.parse(readFileSync(manifest, "utf8")).packageManager;
        if (declared) return declared;
      } catch {}
    }
    if (resolve(directory) === resolve(repository)) return null;
    directory = dirname(directory);
  }
}

/// Every tsconfig that covers an app's own source: `tsconfig.json`, or, for a
/// solution file (`files: []` with references), its referenced configs except
/// the ones named for node/tooling.
function defaultProjects(repository, app) {
  const base = join(repository, app, "tsconfig.json");
  if (!existsSync(base)) return [];
  let config = null;
  try {
    config = JSON.parse(readFileSync(base, "utf8").replace(/\/\*[\s\S]*?\*\/|(^|[^:])\/\/.*$/gm, "$1").replace(/,(\s*[}\]])/g, "$1"));
  } catch {}
  if (config && Array.isArray(config.files) && config.files.length === 0 && Array.isArray(config.references)) {
    return config.references
      .map(reference => String(reference.path))
      .map(path => (path.endsWith(".json") ? path : join(path, "tsconfig.json")))
      .filter(path => !/node|vite|tooling|test/i.test(path))
      .map(path => relative(repository, join(repository, app, path)));
  }
  return [relative(repository, base)];
}

function clone(url, directory, { commit = null, branch = null } = {}) {
  mkdirSync(directory, { recursive: true });
  if (!existsSync(join(directory, ".git"))) {
    git(directory, "init", "-q");
    git(directory, "remote", "add", "origin", url);
  }
  git(directory, "fetch", "-q", "--depth", "1", "origin", commit ?? branch ?? "HEAD");
  git(directory, "checkout", "-q", "--force", "FETCH_HEAD");
  return git(directory, "rev-parse", "HEAD");
}

/// Walks up from `directory` for `node_modules/<name>/package.json`, as the
/// checker's dialect selection does.
function findInstalled(directory, name) {
  for (let current = resolve(directory); ; current = dirname(current)) {
    const manifest = join(current, "node_modules", name, "package.json");
    if (existsSync(manifest)) return manifest;
    if (dirname(current) === current) return null;
  }
}

function readVersion(manifest) {
  try {
    return JSON.parse(readFileSync(manifest, "utf8")).version ?? null;
  } catch {
    return null;
  }
}

/// The Solid runtime a project resolves: `solid-js` from the project
/// directory, `@solidjs/signals` and `@solidjs/web` from `solid-js`'s real path.
function resolvedRuntime(projectDirectory) {
  const solid = findInstalled(projectDirectory, "solid-js");
  if (!solid) return { "solid-js": null };
  const real = dirname(realpathSync(solid));
  const signals = findInstalled(real, "@solidjs/signals");
  const web = findInstalled(projectDirectory, "@solidjs/web") ?? findInstalled(real, "@solidjs/web");
  return { "solid-js": readVersion(solid), "@solidjs/signals": signals && readVersion(signals), "@solidjs/web": web && readVersion(web) };
}

function packageResolver(repository) {
  const cache = new Map();
  return (path, module) => {
    const name = packageOfModule(module);
    const key = `${dirname(path)}\u0000${name}`;
    if (cache.has(key)) return cache.get(key);
    const manifest = findInstalled(dirname(path), name);
    let answer = { package: name, version: null, origin: "unresolved" };
    if (manifest) {
      const real = realpathSync(manifest);
      const inStore = real.split(sep).includes("node_modules");
      answer = { package: name, version: readVersion(real), origin: inStore || !real.startsWith(repository) ? "third-party" : "workspace" };
    }
    cache.set(key, answer);
    return answer;
  };
}

function sourceReader() {
  const cache = new Map();
  return path => {
    if (!cache.has(path)) {
      try {
        cache.set(path, readFileSync(path));
      } catch {
        cache.set(path, null);
      }
    }
    return cache.get(path);
  };
}

const slug = path => path.replace(/[\\/]/g, "__");

/// Annotates every no-contract row whose package, version and specifier the
/// tier has with `environment`: the first recorded dependency-environment
/// entry that Node resolution from the installed package reaches at another
/// version (or not at all), per ADR 0123's admission walk -- from the
/// package's real path, then from each environment package found. Versions
/// only; a pnpm install records no integrity in the installed manifest.
function probeEnvironments(record, repository, tierIndex) {
  const bundles = new Map();
  for (const bundle of tierEntries(tierIndex)) {
    const key = `${bundle.packageName}@${bundle.packageVersion}\u0000${bundle.specifier}`;
    bundles.set(key, [...(bundles.get(key) ?? []), bundle]);
  }
  const memo = new Map();
  for (const project of record.projects ?? []) {
    for (const row of project.rows ?? []) {
      if (row.state !== "no contract" || !row.version) continue;
      const candidates = bundles.get(`${row.package}@${row.version}\u0000${row.module}`);
      if (!candidates) continue;
      const file = join(repository, row.path);
      const memoKey = `${dirname(file)}\u0000${row.package}\u0000${row.module}`;
      if (!memo.has(memoKey)) {
        const manifest = findInstalled(dirname(file), row.package);
        let answer = null;
        if (manifest) {
          const origins = [dirname(realpathSync(manifest))];
          const installed = {};
          const results = candidates.map(bundle => {
            const entries = bundle.solidRuntime ?? tierIndex.environments?.[bundle.dependencyEnvironmentRoot] ?? [];
            for (const entry of entries) {
              if (entry.name in installed) continue;
              let found = null;
              for (const origin of origins) {
                found = findInstalled(origin, entry.name);
                if (found) break;
              }
              installed[entry.name] = found ? { version: readVersion(found) } : null;
              if (found) origins.push(dirname(realpathSync(found)));
            }
            const view = Object.fromEntries(Object.entries(installed).filter(([, value]) => value));
            return { bundle, mismatch: environmentMismatch(entries, view) };
          });
          const admitted = results.find(result => !result.mismatch);
          const runtimeMatched = results.find(result => result.mismatch && !DIALECT_OWNED.includes(result.mismatch.name));
          const chosen = admitted ?? runtimeMatched ?? results[0];
          answer = {
            mismatch: chosen.mismatch,
            bundles: results.length,
            summary: admitted
              ? "every recorded environment version matches the install; the refusal is on integrity or resolution path"
              : `${results.length} bundle(s) for ${row.module}; none matches the install's resolved environment`
          };
        }
        memo.set(memoKey, answer);
      }
      row.environment = memo.get(memoKey);
    }
  }
  return record;
}

function fileDigest(path) {
  return existsSync(path) ? sha256(path) : null;
}

/// Fetches and installs one app; returns `{ ok, refusal?, wallMs, ... }`.
async function prepareApp(app, work, log) {
  const directory = join(work, "apps", app.id);
  const stampPath = join(work, "apps", `${app.id}.install.json`);
  const started = Date.now();
  const url = /^https?:/.test(app.repository) ? app.repository : `https://github.com/${app.repository}`;
  try {
    const head = existsSync(join(directory, ".git")) ? sh("git", ["-C", directory, "rev-parse", "HEAD"]).stdout.trim() : null;
    if (head !== app.commit) clone(url, directory, { commit: app.commit });
  } catch (error) {
    return { ok: false, refusal: `fetch failed: ${error.message}`, wallMs: Date.now() - started };
  }
  const lockfile = join(directory, app.lockfile);
  if (!existsSync(lockfile)) return { ok: false, refusal: `no ${app.lockfile} at ${app.commit}`, wallMs: Date.now() - started };
  const digest = sha256(lockfile);
  if (digest !== app.lockfileDigest) return { ok: false, refusal: `lockfile digest ${digest} differs from the pin`, wallMs: Date.now() - started };
  const install = installCommand(app.lockfile, app.packageManager, readFileSync(lockfile, "utf8").slice(0, 400));
  if (!install) return { ok: false, refusal: `no install command for ${app.lockfile}`, wallMs: Date.now() - started };
  // A reviewed per-app addition (the corpus entry says why), e.g. an engine
  // check the harness's Node does not meet; it never changes the resolved tree.
  install.args.push(...(app.installArgs ?? []));
  const stamp = { commit: app.commit, lockfileDigest: digest, command: [install.command, ...install.args].join(" ") };
  let held = null;
  try {
    held = JSON.parse(readFileSync(stampPath, "utf8"));
  } catch {}
  let installMs = 0;
  if (!(held?.ok && held.commit === stamp.commit && held.lockfileDigest === stamp.lockfileDigest && held.command === stamp.command)) {
    const result = await shAsync(install.command, install.args, {
      cwd: dirname(lockfile),
      env: { ...process.env, CI: "true", COREPACK_ENABLE_STRICT: "0", npm_config_yes: "true", HUSKY: "0" }
    });
    installMs = result.wallMs;
    writeFileSync(join(work, "apps", `${app.id}.install.log`), `${result.stdout}\n${result.stderr}`);
    writeFileSync(stampPath, `${JSON.stringify({ ...stamp, ok: result.ok, wallMs: result.wallMs, manager: install.manager }, null, 2)}\n`);
    if (!result.ok) return { ok: false, refusal: `install failed (${install.manager}): ${(result.stderr || result.stdout).trim().split("\n").slice(-3).join(" / ").slice(0, 300)}`, wallMs: Date.now() - started };
  }
  const runtimes = {};
  for (const project of app.projects) {
    const runtime = resolvedRuntime(join(directory, dirname(project)));
    runtimes[project] = runtime;
    if (!runtime["solid-js"] || !runtime["solid-js"].startsWith("2.")) {
      return { ok: false, refusal: `${project} resolves solid-js ${runtime["solid-js"] ?? "nowhere"}`, wallMs: Date.now() - started, runtimes };
    }
  }
  log(`${app.id}: ready (install ${installMs ? `${(installMs / 1000).toFixed(1)} s` : "cached"})`);
  return { ok: true, directory, runtimes, installMs, wallMs: Date.now() - started, manager: install.manager };
}

function runChecker(checker, typefacts, project, tierOff, extra = []) {
  const args = ["--format", "json", "--project", project, ...(tierOff ? ["--no-bundled-contracts"] : []), ...extra];
  const result = sh(checker, args, {
    env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts },
    timeout: 1800 * 1000
  });
  let output = null;
  try {
    output = JSON.parse(result.stdout);
  } catch {}
  return { output, status: result.status, wallMs: result.wallMs, stderr: String(result.stderr ?? "").slice(-2000) };
}

/// The checker's own per-package contract report (`--check-contracts`) for
/// each project, kept as `contracts: { name: { status, detail } }`: its
/// `detail` is the admission rule's sentence for a compiled-in contract that
/// was not admitted, which the tier comparison alone cannot give (an
/// unreadable lockfile integrity, a patched copy, an environment entry).
/// Attached to every row of the package as `admission`.
function withContractStatuses(record, prepared, options, extractPath) {
  const repository = realpathSync(prepared.directory);
  let changed = false;
  for (const project of record.projects ?? []) {
    if (!project.contracts) {
      const report = runChecker(options.checker, options.typefacts, join(repository, project.project), false, ["--check-contracts"]);
      project.contracts = Object.fromEntries(
        (report.output?.packages ?? []).map(entry => [entry.name, { status: entry.status, detail: entry.detail ?? "" }])
      );
      project.wallMs = { ...(project.wallMs ?? {}), contracts: report.wallMs };
      changed = true;
    }
    for (const row of project.rows ?? []) {
      const detail = project.contracts[row.package]?.detail ?? "";
      const refusal = /was not admitted: (.*)$/.exec(detail)?.[1];
      row.admission = refusal ? [refusal] : [];
    }
  }
  if (changed) writeFileSync(extractPath, `${JSON.stringify(record, null, 1)}\n`);
  return record;
}

/// Analyses one prepared app; writes `<work>/runs/<id>/extract.json`.
function analyseApp(app, prepared, options, log) {
  const runs = join(options.work, "runs", app.id);
  mkdirSync(runs, { recursive: true });
  const identity = { checker: fileDigest(options.checker), typefacts: fileDigest(options.typefacts), commit: app.commit, projects: app.projects };
  const extractPath = join(runs, "extract.json");
  try {
    const held = JSON.parse(readFileSync(extractPath, "utf8"));
    if (JSON.stringify(held.identity) === JSON.stringify(identity) && held.status === "analysed") {
      log(`${app.id}: analysis cached`);
      return withContractStatuses(held, prepared, options, extractPath);
    }
  } catch {}
  const repository = realpathSync(prepared.directory);
  const readSource = sourceReader();
  const resolvePackage = packageResolver(repository);
  const projects = [];
  let wallMs = 0;
  for (const project of app.projects) {
    const path = join(repository, project);
    const on = runChecker(options.checker, options.typefacts, path, false);
    const off = runChecker(options.checker, options.typefacts, path, true);
    wallMs += on.wallMs + off.wallMs;
    writeFileSync(join(runs, `${slug(project)}.on.json`), JSON.stringify(on.output ?? { error: on.stderr }));
    writeFileSync(join(runs, `${slug(project)}.off.json`), JSON.stringify(off.output ?? { error: off.stderr }));
    if (!on.output || !off.output) {
      const refusal = `checker produced no JSON for ${project} (exit ${on.output ? off.status : on.status}): ${(on.output ? off.stderr : on.stderr).trim().split("\n").slice(-2).join(" / ").slice(0, 300)}`;
      const record = { id: app.id, identity, status: "refused", refusal, wallMs };
      writeFileSync(extractPath, `${JSON.stringify(record, null, 1)}\n`);
      return record;
    }
    const refusedRuntime = (on.output.findings ?? []).find(finding => finding.id === "SC9013");
    const classified = classifyProject({ on: on.output, off: off.output, readSource, resolvePackage });
    const relativise = row => ({ ...row, path: relative(repository, row.path) });
    projects.push({
      project,
      status: { on: on.output.status, off: off.output.status },
      refusedRuntime: refusedRuntime ? refusedRuntime.message.slice(0, 300) : null,
      wallMs: { on: on.wallMs, off: off.wallMs },
      findings: { on: (on.output.findings ?? []).length, off: (off.output.findings ?? []).length },
      rows: classified.rows.map(relativise),
      onOnly: classified.onOnly.map(relativise),
      projectCallbackExecution: classified.projectCallbackExecution,
      accepted: classified.accepted,
      filesAnalyzed: on.output.metrics?.filesAnalyzed ?? null
    });
  }
  const record = { id: app.id, identity, status: "analysed", solid: prepared.runtimes, projects, wallMs };
  writeFileSync(extractPath, `${JSON.stringify(record, null, 1)}\n`);
  log(`${app.id}: analysed ${projects.length} project(s) in ${(wallMs / 1000).toFixed(1)} s`);
  return withContractStatuses(record, prepared, options, extractPath);
}

async function pool(items, jobs, work) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.max(1, jobs) }, async () => {
      while (next < items.length) {
        const index = next++;
        results[index] = await work(items[index], index);
      }
    })
  );
  return results;
}

// ---------------------------------------------------------------------------
// Pinning (network)
// ---------------------------------------------------------------------------

function pin(options) {
  const draft = JSON.parse(readFileSync(options.pin, "utf8"));
  const corpus = existsSync(options.corpus) ? JSON.parse(readFileSync(options.corpus, "utf8")) : { schemaVersion: 1, apps: [], excluded: [] };
  const byId = new Map(corpus.apps.map(app => [app.id, app]));
  for (const entry of draft.apps ?? []) {
    if (byId.has(entry.id) && !entry.repin) continue;
    const directory = join(options.work, "apps", entry.id);
    const url = `https://github.com/${entry.repository}`;
    try {
      const commit = clone(url, directory, { commit: entry.commit ?? null, branch: entry.branch ?? null });
      const commitDate = git(directory, "log", "-1", "--format=%cI");
      const lockfile = findLockfile(directory, entry.app ?? ".");
      if (!lockfile) throw new Error("no lockfile at or above the app");
      const projects = entry.projects ?? defaultProjects(directory, entry.app ?? ".");
      if (projects.length === 0) throw new Error("no tsconfig.json in the app directory");
      const packageManager = nearestPackageManager(directory, lockfile);
      byId.set(entry.id, {
        id: entry.id,
        repository: entry.repository,
        ...(entry.branch ? { branch: entry.branch } : {}),
        commit,
        commitDate,
        app: entry.app ?? ".",
        projects,
        lockfile,
        lockfileDigest: sha256(join(directory, lockfile)),
        ...(packageManager ? { packageManager } : {}),
        why: entry.why
      });
      console.error(`pinned ${entry.id} at ${commit.slice(0, 8)} (${lockfile}, ${projects.join(", ")})`);
    } catch (error) {
      console.error(`${entry.id}: ${error.message}`);
      corpus.excluded = [...(corpus.excluded ?? []).filter(row => row.repository !== entry.repository || row.app !== (entry.app ?? ".")), { repository: entry.repository, app: entry.app ?? ".", reason: `pin failed: ${error.message.slice(0, 200)}` }];
    }
  }
  corpus.apps = [...byId.values()];
  corpus.pinnedAt = new Date().toISOString().slice(0, 10);
  writeFileSync(options.corpus, `${JSON.stringify(corpus, null, 2)}\n`);
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArguments(argv) {
  const options = {
    mode: null,
    pin: null,
    corpus: CORPUS_PATH,
    work: DEFAULT_WORK,
    only: null,
    checker: join(root, "rust/target/release/solid-checker-rust"),
    typefacts: join(root, "bin/solid-typefacts"),
    packageMetric: null,
    jobs: 4,
    json: null,
    markdown: null,
    clean: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--pin") (options.mode = "pin"), (options.pin = resolve(argv[++index]));
    else if (argument === "--run") options.mode = "run";
    else if (argument === "--measure") options.mode = "measure";
    else if (argument === "--corpus") options.corpus = resolve(argv[++index]);
    else if (argument === "--work") options.work = resolve(argv[++index]);
    else if (argument === "--only") options.only = new Set(argv[++index].split(","));
    else if (argument === "--checker") options.checker = resolve(argv[++index]);
    else if (argument === "--typefacts") options.typefacts = resolve(argv[++index]);
    else if (argument === "--package-metric") options.packageMetric = resolve(argv[++index]);
    else if (argument === "--jobs") options.jobs = Number(argv[++index]);
    else if (argument === "--json") options.json = resolve(argv[++index]);
    else if (argument === "--markdown") options.markdown = resolve(argv[++index]);
    else if (argument === "--clean") options.clean = true;
    else if (argument === "-h" || argument === "--help") {
      console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n\n")[1]);
      process.exit(0);
    } else fail(`unknown argument ${JSON.stringify(argument)}`);
  }
  if (!options.mode) fail("one of --pin <draft.json>, --run or --measure is required");
  options.json ??= join(options.work, "metric.json");
  options.markdown ??= join(options.work, "metric.md");
  return options;
}

/// The lockfile's format as the checker's integrity readers see it: the
/// file name, pnpm's document count, bun's `lockfileVersion`.
function lockfileShape(path) {
  const name = path.split("/").pop();
  if (name === "bun.lockb") return "bun.lockb (binary)";
  let text = "";
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return name;
  }
  if (name === "pnpm-lock.yaml") {
    const documents = text.split("\n").filter(line => line.trim() === "---").length;
    return documents ? `pnpm-lock.yaml with a YAML document marker (${documents})` : "pnpm-lock.yaml (one document)";
  }
  if (name === "bun.lock") return `bun.lock lockfileVersion ${/"lockfileVersion":\s*(\d+)/.exec(text)?.[1] ?? "?"}`;
  return name;
}

/// Re-probes the environments of an analysed app while its clone exists, and
/// saves the answer beside the extracted sites so `--measure` keeps it after
/// `--clean`.
function annotate(record, app, options) {
  const directory = join(options.work, "apps", app.id);
  if (record.status !== "analysed" || !existsSync(directory)) return record;
  record.lockfileShape = lockfileShape(join(directory, app.lockfile));
  probeEnvironments(record, realpathSync(directory), JSON.parse(readFileSync(TIER_INDEX_PATH, "utf8")));
  writeFileSync(join(options.work, "runs", app.id, "extract.json"), `${JSON.stringify(record, null, 1)}\n`);
  return record;
}

function writeMeasurement(options, apps, extra = {}) {
  const tierIndex = JSON.parse(readFileSync(TIER_INDEX_PATH, "utf8"));
  const packageMetric = options.packageMetric ? JSON.parse(readFileSync(options.packageMetric, "utf8")) : null;
  const result = { ...measure({ apps, tierIndex, packageMetric }), ...extra, packageMetric: options.packageMetric ? relative(root, options.packageMetric) : null };
  mkdirSync(dirname(options.json), { recursive: true });
  writeFileSync(options.json, `${JSON.stringify(result, null, 2)}\n`);
  const markdown = renderMarkdown(result);
  writeFileSync(options.markdown, markdown);
  console.log(markdown);
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  mkdirSync(join(options.work, "apps"), { recursive: true });
  if (options.mode === "pin") return pin(options);
  const corpus = JSON.parse(readFileSync(options.corpus, "utf8"));
  const selected = corpus.apps.filter(app => !options.only || options.only.has(app.id));
  const log = message => console.error(`app-import-metric: ${message}`);
  if (options.mode === "measure") {
    const apps = selected.map(app => {
      try {
        const record = JSON.parse(readFileSync(join(options.work, "runs", app.id, "extract.json"), "utf8"));
        return annotate(record, app, options);
      } catch {
        return { id: app.id, status: "not run" };
      }
    });
    return writeMeasurement(options, apps);
  }
  if (!existsSync(options.checker)) fail(`no checker at ${options.checker}; run make build-checker-release`);
  const started = Date.now();
  // Installs run in parallel; analyses run one app at a time so that the
  // checker's walls are not inflated by each other.
  const prepared = await pool(selected, options.jobs, async app => {
    const result = await prepareApp(app, options.work, log);
    if (!result.ok) log(`${app.id}: refused: ${result.refusal}`);
    return result;
  });
  const prepareMs = Date.now() - started;
  const apps = [];
  for (const [index, app] of selected.entries()) {
    const ready = prepared[index];
    if (!ready.ok) {
      apps.push({ id: app.id, status: "refused", refusal: ready.refusal, solid: ready.runtimes ?? null });
      continue;
    }
    apps.push(annotate(analyseApp(app, ready, options, log), app, options));
  }
  const totalMs = Date.now() - started;
  writeMeasurement(options, apps, {
    run: {
      checker: relative(root, options.checker),
      checkerDigest: fileDigest(options.checker),
      typefactsDigest: fileDigest(options.typefacts),
      prepareMs,
      analyseMs: totalMs - prepareMs,
      totalMs
    }
  });
  if (options.clean) {
    for (const app of selected) rmSync(join(options.work, "apps", app.id), { recursive: true, force: true });
    log(`removed ${selected.length} clone(s) under ${join(options.work, "apps")}`);
  }
}

if (import.meta.main) await main();
