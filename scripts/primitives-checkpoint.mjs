#!/usr/bin/env bun

// The @solid-primitives checkpoint: for every @solid-primitives package with a
// Solid 2 release, is its contract generated and certified per host, is every
// export accounted for, and does misuse of each primitive report the right
// finding?
//
//   bun scripts/primitives-checkpoint.mjs --select [--no-branch]
//                                        network: re-pin the corpus
//   bun scripts/primitives-checkpoint.mjs --print-probes    probe ids, one per line
//   bun scripts/primitives-checkpoint.mjs --measure <run.json> --json <out> [--clean-retained]
//                                        one host's certification, measured
//   bun scripts/primitives-checkpoint.mjs --misuse --json <out> [--case <id>]... [--package <name>]... [--concurrency <n>]
//                                        network: evaluate the misuse ledger
//   bun scripts/primitives-checkpoint.mjs --report <measure.json,...> [--misuse-results <json>]
//                                        [--json <out>] [--markdown <out>]
//
// `make primitives-checkpoint` certifies the corpus host free and once per
// host (ADR 0140) with the benchmark runner, measures each run here, evaluates
// the misuse ledger, and writes the report. Nothing here certifies anything.
//
// # The checkpoint (owner, 2026-09-28)
//
// A package is at the checkpoint when all of these hold:
//
//   1  its contract certifies from the published bytes host free and for
//      `browser` and `node`, and every nameable entrypoint is in the authored
//      tier (`pkg/contracts/authored/index.json`) for each of the three (the
//      certified tier this once read is retired, ADR 0228);
//   2  every export is certified clean in every host, or is uncertifiable only
//      for a reason the owner has accepted as genuinely unprovable
//      (`ACCEPTED_UNPROVABLE`, empty until the owner names one: no current
//      blocking cause is one, every one of them is a checker gap), or belongs
//      to a package whose published bytes cannot load under the pinned runtime
//      in any host (a published package defect, owner ruling 2026-09-30):
//      uncertifiable with the agreed reason "published package defect",
//      nothing claimed about it, never counted clean;
//   3  every export with a misuse path has a ledger case
//      (`fixtures/primitives-misuse/cases.json`) that, against the real
//      published typings, reports the expected rule on the misuse and nothing
//      on the correct use, with `tsc` silent on both, in every host the case
//      names;
//   4  the app-import metric shows its import sites certified. That metric is
//      separate measurement tooling and is not read here yet; the report says so.
//
// # Measurement reuse
//
// Per host, `certification-metric.mjs`'s `readRetainedRow` and `measure` read
// the retained benchmark trees exactly as the certification metric does, so an
// export's bucket and its blocking causes mean the same thing in both reports.
// This script adds what the metric does not carry: per-host certification and
// tier presence, and the misuse paths each export has.
//
// # Misuse paths
//
// A misuse path is a way a caller can use an export wrongly that a catalog
// rule is meant to report. It is derived, strongest first, from
//
//   contract  an operation the certified or generated (proposed) summary states:
//             an owner requirement (SC4001 missing-owner when called outside an
//             owner), a returned accessor or reading callable (SC1001
//             strict-read-untracked when read at a component's top level), a
//             callback invoked synchronously untracked (SC1001 for a read inside
//             it) or tracked (SC2001 reactive-write-in-owned-scope for a write
//             inside it), an argument read synchronously untracked (SC1001);
//   types     the published declaration, where no summary says anything: a
//             signature returning an accessor-shaped callable, taking a
//             callback, or taking an accessor argument. A types-only path is a
//             hint that names the rule a claim would feed; it is not a claim.
//
// A proposed operation is what the generator derived before certification
// weakened it, so a path from it is a path the export *has*, even while the
// accepted summary cannot say so yet.

import { execFile, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { availableParallelism } from "node:os";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";

import { corpusProblems, measure, readRetainedRow } from "./certification-metric.mjs";

const root = resolve(import.meta.dirname, "..");
export const CORPUS_PATH = join(root, "scripts/ecosystem-benchmark/primitives-checkpoint-corpus.json");
const MANIFEST_PATH = join(root, "scripts/ecosystem-benchmark/manifest.json");
const TIER_INDEX_PATH = join(root, "pkg/contracts/authored/index.json");
export const MISUSE_LEDGER_PATH = join(root, "fixtures/primitives-misuse/cases.json");
const METRIC_CORPUS_PATH = join(root, "scripts/ecosystem-benchmark/certification-metric-corpus.json");

export const SCOPE = "solid-primitives";
export const HOSTS = ["none", "browser", "node"];
const DOMAINS = ["callbacks", "reads", "returns", "creates"];

/// Blocking causes the owner has accepted as genuinely unprovable, as
/// `{ class, key }`. Empty: every cause the classifier names today is a
/// checker gap (a missing claim form, recipe, census refusal, decline or
/// dialect row), and an export it blocks is not accounted for.
export const ACCEPTED_UNPROVABLE = [];

function fail(message) {
  console.error(`primitives-checkpoint: ${message}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// The Solid 2 releases of one packument: every published version whose
/// declared `solid-js` range (runtime before peer) admits a published Solid 2
/// release, or that depends on `@solidjs/signals`. `solidVersions` is the
/// list of published `solid-js` 2.x versions to test ranges against.
export function solid2Releases(packument, solidVersions, { satisfies, flattenRanges, solidRanges }) {
  const releases = [];
  for (const [version, document] of Object.entries(packument?.versions ?? {})) {
    const ranges = flattenRanges(solidRanges(document, ["solid-js", "@solidjs/signals"]));
    const solid = ranges["solid-js"] ?? null;
    const admitsSolid2 = solid !== null && solidVersions.some(candidate => satisfies(candidate, solid));
    if (admitsSolid2 || ranges["@solidjs/signals"]) {
      releases.push({ version, solid, signals: ranges["@solidjs/signals"] ?? null, integrity: document?.dist?.integrity ?? null });
    }
  }
  return releases;
}

/// Refuses a manifest that disagrees with the pin, as the metric's corpus
/// check does, plus a manifest Solid 2 row of the scope the pin left out.
export function checkpointCorpusProblems(corpus, manifest) {
  const problems = corpusProblems(corpus, manifest);
  const pinned = new Set((corpus?.packages ?? []).map(entry => entry.package));
  for (const row of manifest?.rows ?? []) {
    if (row.solidTarget !== "solid2" || !row.package.startsWith(`@${SCOPE}/`)) continue;
    if (!pinned.has(row.package)) problems.push(`${row.package}: a solid2 manifest row the corpus does not pin`);
  }
  return problems;
}

async function select(corpusPath, { branch = true } = {}) {
  const { Registry, flattenRanges, solidRanges } = await import("./ecosystem-benchmark/lib/registry.mjs");
  const { satisfies, sortVersions } = await import("./ecosystem-benchmark/lib/semver.mjs");
  const registry = new Registry();
  const manifest = JSON.parse(readFileSync(MANIFEST_PATH, "utf8"));
  const metricCorpus = JSON.parse(readFileSync(METRIC_CORPUS_PATH, "utf8"));
  const census = execFileSync("bun", [join(root, "scripts/contract-coverage-census.mjs"), "--print-packages"], { encoding: "utf8" })
    .split("\n")
    .filter(Boolean);
  const names = await registry.orgPackages(SCOPE);
  if (names.length === 0) fail(`the registry lists no packages in the @${SCOPE} org`);
  const solidPackument = await registry.packument("solid-js");
  const solidVersions = Object.keys(solidPackument?.versions ?? {}).filter(version => version.startsWith("2."));
  // The upstream `next` branch: its package directories and their unpublished
  // versions. Informational; the registry is the fact the pin is taken from.
  const branchVersions = new Map();
  if (branch) {
    const listing = JSON.parse(
      execFileSync("gh", ["api", "repos/solidjs-community/solid-primitives/contents/packages?ref=next"], { encoding: "utf8" })
    );
    for (const entry of listing.filter(item => item.type === "dir")) {
      const file = JSON.parse(
        execFileSync("gh", ["api", `repos/solidjs-community/solid-primitives/contents/packages/${entry.name}/package.json?ref=next`], {
          encoding: "utf8"
        })
      );
      const manifestText = Buffer.from(file.content, "base64").toString("utf8");
      const packageJson = JSON.parse(manifestText);
      branchVersions.set(packageJson.name, { directory: entry.name, version: packageJson.version, solidPeer: packageJson.peerDependencies?.["solid-js"] ?? null });
    }
  }
  const packages = [];
  const withoutSolid2 = [];
  const problems = [];
  const packuments = await registry.mapConcurrent(names, name => registry.packument(name));
  names.forEach((name, index) => {
    const packument = packuments[index];
    const releases = solid2Releases(packument, solidVersions, { satisfies, flattenRanges, solidRanges });
    const tags = packument?.["dist-tags"] ?? {};
    const branchEntry = branchVersions.get(name) ?? null;
    if (releases.length === 0) {
      withoutSolid2.push({ package: name, distTags: tags, nextBranch: branchEntry });
      return;
    }
    const pinned = sortVersions(releases.map(release => release.version)).at(-1);
    const release = releases.find(entry => entry.version === pinned);
    const row = manifest.rows.find(candidate => candidate.solidTarget === "solid2" && candidate.package === name);
    const probe = row?.probes?.find(candidate => candidate.kind === "head") ?? null;
    if (!row) problems.push(`${name}: Solid 2 release ${pinned} has no solid2 manifest row (run make ecosystem-discover)`);
    else if (row.version !== pinned || row.integrity !== release.integrity) {
      problems.push(`${name}: the registry's newest Solid 2 release is ${pinned}, the manifest row pins ${row.version}`);
    } else if (!probe) problems.push(`${name}: the manifest row has no head probe`);
    packages.push({
      rank: 0,
      package: name,
      version: pinned,
      integrity: release.integrity,
      family: SCOPE,
      weeklyDownloads: null,
      probe: probe?.id ?? null,
      probeKind: probe?.kind ?? null,
      solid: probe?.solid ?? null,
      solidPeer: release.solid,
      nextDistTag: tags.next ?? null,
      solid2Releases: releases.length,
      nextBranch: branchEntry,
      inMetricCorpus: metricCorpus.packages.some(entry => entry.package === name),
      inCensus: census.includes(name)
    });
  });
  if (problems.length) fail(`the manifest does not agree with the registry:\n  ${problems.join("\n  ")}`);
  packages.sort((left, right) => left.package.localeCompare(right.package));
  packages.forEach((entry, index) => {
    entry.rank = index + 1;
  });
  const corpus = {
    format: "solid-checker-primitives-checkpoint-corpus",
    corpusVersion: 1,
    measuredOn: new Date().toISOString().slice(0, 10),
    scope: `@${SCOPE}`,
    source: {
      org: `${registry.registry}/-/org/${SCOPE}/package (${names.length} packages)`,
      solid2: "a published version whose solid-js range (runtime before peer) admits a published solid-js 2.x, or that depends on @solidjs/signals; the newest such version is pinned",
      manifest: { path: "scripts/ecosystem-benchmark/manifest.json", generatedAt: manifest.generatedAt ?? null },
      nextBranch: branch ? "gh api repos/solidjs-community/solid-primitives/contents/packages?ref=next (informational: unpublished versions)" : null
    },
    packages,
    withoutSolid2
  };
  writeFileSync(corpusPath, `${JSON.stringify(corpus, null, 2)}\n`);
  console.log(`pinned ${packages.length} packages with a Solid 2 release (${withoutSolid2.length} without) to ${corpusPath}`);
}

// ---------------------------------------------------------------------------
// Misuse paths
// ---------------------------------------------------------------------------

export const MISUSE_RULES = {
  owner: { rule: "missing-owner", code: "SC4001", misuse: "called outside an owner (module scope, a detached callback)" },
  returnedAccessor: { rule: "strict-read-untracked", code: "SC1001", misuse: "the returned accessor read at a component's top level" },
  untrackedCallback: { rule: "strict-read-untracked", code: "SC1001", misuse: "a reactive read inside a callback the export runs synchronously, untracked" },
  trackedCallback: { rule: "reactive-write-in-owned-scope", code: "SC2001", misuse: "a signal write inside a callback the export runs tracked" },
  argumentRead: { rule: "strict-read-untracked", code: "SC1001", misuse: "called at a component's top level, where it reads its accessor argument untracked" },
  typeAccessorReturn: { rule: "strict-read-untracked", code: "SC1001", misuse: "types only: returns an accessor-shaped callable; a claim would feed a top-level read" },
  typeCallback: { rule: null, code: null, misuse: "types only: takes a callback whose scope no claim states" },
  typeAccessorArgument: { rule: "strict-read-untracked", code: "SC1001", misuse: "types only: takes an accessor argument no claim says it reads" }
};

/// Whether a wire output is, or carries, something a caller reads as an
/// accessor: a reactive accessor, or a described callable that reads.
function outputCarriesRead(output) {
  if (!output || typeof output !== "object") return false;
  if (output.kind === "reactive" && output.role === "accessor") return true;
  if (output.kind === "described-callable" && (output.reads ?? []).length > 0) return true;
  return [...(output.properties ?? []).map(property => property?.value), ...(output.items ?? [])].some(outputCarriesRead);
}

const synchronous = operation => (operation.at?.schedule ?? "same-stack") === "same-stack";

/// The contract-derived misuse classes of one summary.
export function contractMisuse(summary) {
  const classes = new Set();
  for (const operation of summary?.call?.operations ?? []) {
    const relation = operation.owner ?? {};
    if (relation.requires === "required" || relation.requiresCleanup === "required") classes.add("owner");
    if (operation.kind === "return" && outputCarriesRead(operation.output)) classes.add("returnedAccessor");
    if (operation.kind === "invoke" && (operation.protocol ?? "call") === "call" && synchronous(operation)) {
      if (operation.tracking === "untracked") classes.add("untrackedCallback");
      if (operation.tracking === "tracked") classes.add("trackedCallback");
      // An inline invocation inherits the caller's listener unless the
      // contract states otherwise. Passing an accessor at a component's top
      // level therefore has an argument-read misuse path (utils.access),
      // even though the package performs no package-owned reactive read.
      if (
        operation.tracking === "ambient-at-execution" &&
        operation.at?.event === "call" &&
        operation.trigger?.event === "call" &&
        (summary.call.callbacks ?? []).some(callback =>
          callback.operation === operation.id &&
          Number.isInteger(callback.from?.arg) && callback.from.arg >= 0 &&
          (callback.from.path ?? []).length === 0
        )
      ) classes.add("argumentRead");
    }
    if (
      operation.kind === "read" &&
      operation.tracking === "untracked" &&
      synchronous(operation) &&
      (operation.inputs ?? []).some(input => input?.kind === "parameter")
    ) {
      classes.add("argumentRead");
    }
  }
  return classes;
}

/// `(entrypoint, export)` -> { classes, sources } from contract documents:
/// the accepted documents (source `accepted`) and the generated proposal
/// (source `proposed`), across every artifact case.
export function contractMisusePaths(documents) {
  const paths = new Map();
  for (const { document, source } of documents) {
    for (const [entrypoint, value] of Object.entries(document?.entrypoints ?? {})) {
      for (const artifactCase of value.cases ?? value ?? []) {
        for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
          const id = typeof reference === "string" ? reference : reference?.summary;
          const classes = contractMisuse(document.summaries?.[id]);
          if (classes.size === 0) continue;
          const key = `${entrypoint}\u0000${exportName}`;
          const held = paths.get(key) ?? new Map();
          for (const kind of classes) {
            held.set(kind, held.get(kind) === "accepted" || source === "accepted" ? "accepted" : "proposed");
          }
          paths.set(key, held);
        }
      }
    }
  }
  return paths;
}

let typescript = null;
function loadTypeScript() {
  typescript ??= createRequire(join(root, "packages/cli/package.json"))("typescript");
  return typescript;
}

/// Types-only misuse classes of one export type. `isLibrary(declaration)`
/// says whether a declaration belongs to TypeScript's default library, whose
/// types (arrays, maps, promises) are never looked into: `Array#pop` is a
/// zero-argument callable, not an accessor.
export function typeMisuse(ts, checker, type, isLibrary = () => false) {
  const classes = new Set();
  const accessorLike = candidate =>
    candidate
      .getCallSignatures()
      .some(signature => signature.parameters.every(parameter => (parameter.valueDeclaration?.questionToken ?? null) !== null || parameter.valueDeclaration?.initializer) && !(signature.getReturnType().flags & ts.TypeFlags.Void));
  const carriesAccessor = (candidate, depth = 0) => {
    if (accessorLike(candidate)) return true;
    if (depth > 1) return false;
    if (checker.isTupleType?.(candidate)) return checker.getTypeArguments(candidate).some(item => carriesAccessor(item, depth + 1));
    if (candidate.isUnion()) return candidate.types.some(item => carriesAccessor(item, depth + 1));
    if (checker.isArrayType?.(candidate)) return false;
    const declared = candidate.getSymbol()?.declarations ?? [];
    if (declared.length > 0 && declared.every(isLibrary)) return false;
    if (candidate.flags & ts.TypeFlags.Object && candidate.getCallSignatures().length === 0) {
      return candidate.getProperties().slice(0, 40).some(property => {
        const declaration = property.valueDeclaration ?? property.declarations?.[0];
        return declaration ? carriesAccessor(checker.getTypeOfSymbolAtLocation(property, declaration), depth + 1) : false;
      });
    }
    return false;
  };
  for (const signature of type.getCallSignatures()) {
    if (carriesAccessor(signature.getReturnType())) classes.add("typeAccessorReturn");
    for (const parameter of signature.parameters) {
      const declaration = parameter.valueDeclaration;
      if (!declaration) continue;
      const parameterType = checker.getNonNullableType(checker.getTypeOfSymbolAtLocation(parameter, declaration));
      const alternatives = parameterType.isUnion() ? parameterType.types : [parameterType];
      for (const alternative of alternatives) {
        const signatures = alternative.getCallSignatures();
        if (signatures.length === 0) continue;
        if (signatures.some(candidate => candidate.parameters.length === 0 && !(candidate.getReturnType().flags & ts.TypeFlags.Void))) {
          classes.add("typeAccessorArgument");
        } else {
          classes.add("typeCallback");
        }
      }
    }
  }
  return classes;
}

/// `(entrypoint, export)` -> types-only classes, read from the declarations
/// each generated case names, inside the retained install project.
export function typeMisusePaths(generated, projectDir) {
  const paths = new Map();
  if (!generated || !projectDir || !existsSync(projectDir)) return paths;
  const ts = loadTypeScript();
  const packageRoot = join(projectDir, "node_modules", generated.package.name);
  const declarations = new Map();
  for (const [entrypoint, value] of Object.entries(generated.entrypoints ?? {})) {
    for (const artifactCase of value.cases ?? value ?? []) {
      const path = artifactCase.declarations?.path;
      if (!path) continue;
      const file = join(packageRoot, path);
      if (!existsSync(file)) continue;
      const list = declarations.get(file) ?? new Set();
      list.add(entrypoint);
      declarations.set(file, list);
    }
  }
  if (declarations.size === 0) return paths;
  const program = ts.createProgram([...declarations.keys()], {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    jsx: ts.JsxEmit.Preserve,
    jsxImportSource: "@solidjs/web",
    skipLibCheck: true,
    strict: true,
    noEmit: true,
    types: []
  });
  const checker = program.getTypeChecker();
  for (const [file, entrypoints] of declarations) {
    const source = program.getSourceFile(file);
    const moduleSymbol = source ? checker.getSymbolAtLocation(source) : null;
    if (!moduleSymbol) continue;
    for (const exported of checker.getExportsOfModule(moduleSymbol)) {
      const symbol = exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
      if (!(symbol.flags & ts.SymbolFlags.Value)) continue;
      const declaration = symbol.valueDeclaration ?? symbol.declarations?.[0];
      if (!declaration) continue;
      const classes = typeMisuse(ts, checker, checker.getTypeOfSymbolAtLocation(symbol, declaration), node =>
        program.isSourceFileDefaultLibrary(node.getSourceFile())
      );
      if (classes.size === 0) continue;
      for (const entrypoint of entrypoints) paths.set(`${entrypoint}\u0000${exported.name}`, classes);
    }
  }
  return paths;
}

/// One export's misuse paths: contract classes first; types-only classes
/// only where no contract class exists for that export.
export function misusePathsOf(key, contractPaths, typePaths) {
  const contract = contractPaths.get(key);
  if (contract && contract.size > 0) {
    return [...contract].map(([kind, source]) => ({ class: kind, source, ...MISUSE_RULES[kind] })).sort((a, b) => a.class.localeCompare(b.class));
  }
  const types = typePaths.get(key);
  if (!types) return [];
  return [...types].sort().map(kind => ({ class: kind, source: "types", ...MISUSE_RULES[kind] }));
}

// ---------------------------------------------------------------------------
// One host
// ---------------------------------------------------------------------------

const scrub = text => String(text ?? "").replace(/(?:\/private)?\/var\/folders\/\S+|\/private\/tmp\/\S+/g, "<tmp>");

/// Why a row did not certify, without machine-specific temporary paths; null
/// for a certified row.
export function refusalOf(result) {
  const attempt = result?.certificationAttempt ?? null;
  if (attempt?.status === "certified") return null;
  const reason = typeof attempt?.reason === "string" && attempt.reason ? attempt.reason : result?.signature ?? result?.class ?? null;
  return reason === null ? null : scrub(reason).slice(0, 300);
}

/// Why the published-graph lane prepared nothing for a row, as one wall key.
/// A refused graph lane leaves the row on the plain lane, whose exports are
/// then held behind the dependency the graph would have accepted; the report
/// attributes those holds to this key.
export function graphRefusalOf(result) {
  // Two trace shapes carry the refusal: a declined-only frontier whose
  // composition failed (`preparationRefusal`), and a partial proposal whose
  // graph preparation was attempted and failed (`partialProposalFrontier:
  // "unprepared"`, `reason`). Reading only the first left the second's
  // dependents attributed to the dependency the graph never reached.
  const preparation = result?.certificationAttempt?.graphPreparation;
  const text =
    preparation?.preparationRefusal ??
    (preparation?.partialProposalFrontier === "unprepared" ? preparation.reason : undefined);
  if (typeof text !== "string" || !text) return null;
  const unbound = /graph node (\S+) \S+ \[([^\]]*)\] refused:.*Declaration target for export "([^"]+)" is re-exported from dependency "([^"]+)"/.exec(text);
  if (unbound) {
    return { key: `graph node ${unbound[1]} [${unbound[2]}]: export "${unbound[3]}" re-exported from ${unbound[4]}, which no planned dependency binds` };
  }
  // A dependent node whose re-export reaches a name its dependency's contract
  // withholds (ADR 0128's unbound, ADR 0150's foreign declaration exports).
  const withheld = /graph node (\S+) \S+ \[([^\]]*)\] refused:.*accepted dependency (\S+) has no exact (runtime|declarations) binding for export (\S+)/.exec(text);
  if (withheld) {
    return { key: `graph node ${withheld[1]} [${withheld[2]}]: export "${withheld[5]}" has no exact ${withheld[4]} binding in ${withheld[3]}` };
  }
  const notExported = /prepared no artifact case: (\S+) is not exported by the package/.exec(text);
  if (notExported) return { key: `${notExported[1]} is not exported by the imported package` };
  const notInstalled = /prepared no artifact case: (\S+) is not installed above \S*node_modules\/((?:@[^/]+\/)?[^/]+)\//.exec(text);
  if (notInstalled) return { key: `${notInstalled[1]} is not installed (imported by ${notInstalled[2]})` };
  return { key: scrub(text).replace(/^published dependency graph prepared no artifact case:\s*/, "").slice(0, 120) };
}

/// A refusal that is a fact about the published bytes under the pinned
/// runtime, not a checker gap: the package cannot load there at all. Whether
/// such a package leaves the checkpoint's denominator is an owner decision.
export function publishedDefectOf(result) {
  const texts = [refusalOf(result), ...(result?.artifactCaseRefusals ?? []).map(entry => entry.reason)].filter(Boolean).map(String);
  const missing = texts.map(text => /resolved target <package-root>\/(\S+) is not a file/.exec(text)?.[1]).find(Boolean);
  if (result?.class === "unavailable-published-target" && missing) {
    return { kind: "runtime target not published", detail: `${missing} is not in the tarball` };
  }
  const notExported = texts.map(text => /dependency-target-not-exported: (\S+) is not exported by (\S+)/.exec(text)).find(Boolean);
  if (notExported) return { kind: "imports a specifier the runtime does not export", detail: `${notExported[1]} (${notExported[2]})` };
  return null;
}

/// Measures one host's run: the certification metric's own measurement over
/// the checkpoint corpus, plus each package's certification outcome and each
/// export's misuse paths.
export function measureHost({ run, corpus, rows, typePaths = new Map() }) {
  const metric = measure({ run, corpus, rows, demandRows: [] });
  const byProbe = new Map((run.results ?? []).map(result => [result.probeId, result]));
  const packages = metric.packages.map(entry => {
    const corpusEntry = corpus.packages.find(candidate => candidate.probe === entry.probe);
    const row = rows.get(entry.probe) ?? null;
    const result = byProbe.get(entry.probe) ?? {};
    const documents = [
      ...(row?.generated ? [{ document: row.generated, source: "proposed" }] : []),
      ...(row?.documents ?? []).filter(item => item.document.package?.name === entry.package).map(item => ({ document: item.document, source: "accepted" }))
    ];
    const contractPaths = contractMisusePaths(documents);
    const types = typePaths.get(entry.probe) ?? new Map();
    return {
      package: entry.package,
      version: entry.version,
      probe: entry.probe,
      rowClass: entry.rowClass,
      certification: entry.certification,
      stage: result.certificationAttempt?.stage ?? null,
      reason: refusalOf(result),
      publishedDefect: publishedDefectOf(result),
      graphRefusal: graphRefusalOf(result),
      lane: entry.lane,
      certifiable: entry.certifiable,
      refusedEntrypoints: entry.refusedEntrypoints,
      counts: entry.counts,
      total: entry.total,
      inMetricCorpus: corpusEntry?.inMetricCorpus ?? false,
      exports: entry.exports.map(item => ({
        entrypoint: item.entrypoint,
        export: item.export,
        bucket: item.bucket,
        causes: item.causes.map(cause => ({
          domain: cause.domain ?? null,
          class: cause.class,
          key: cause.key,
          ...(cause.callee ? { callee: cause.callee } : {}),
          ...(cause.class === "unaccepted dependency" && graphRefusalOf(result) ? { behind: graphRefusalOf(result).key } : {})
        })),
        misuse: misusePathsOf(`${item.entrypoint}\u0000${item.export}`, contractPaths, types)
      }))
    };
  });
  return {
    format: "solid-checker-primitives-checkpoint-host",
    version: 1,
    host: metric.host ?? "none",
    run: metric.run,
    missingProbes: metric.missingProbes,
    headline: metric.headline,
    totals: metric.totals,
    classes: metric.classes,
    walls: metric.walls.map(wall => ({ class: wall.class, key: wall.key, exports: wall.exports, sole: wall.sole, packages: wall.packages, examples: wall.examples.slice(0, 3) })),
    unlockCurve: metric.unlockCurve,
    packages
  };
}

// ---------------------------------------------------------------------------
// The compiled-in tier
// ---------------------------------------------------------------------------

export function hostOfConditions(conditions) {
  if ((conditions ?? []).includes("browser")) return "browser";
  if ((conditions ?? []).includes("node")) return "node";
  return "none";
}

/// The Solid runtime packages an entry's recorded environment must
/// resolve to the corpus install's release, and which pin each one follows.
/// `@solidjs/signals` has no pin of its own: the head probe's install holds it
/// at the `solid-js` release, as the misuse runner's install does.
const RUNTIME_PINS = Object.freeze({ "solid-js": "solid-js", "@solidjs/web": "@solidjs/web", "@solidjs/signals": "solid-js" });

/// Whether an entry's recorded environment is the runtime `pin` installs:
/// every entry of every runtime package the environment records resolves to
/// the pinned release. A consumer admits an entry only in the environment it
/// recorded (ADRs 0123, 0126), so an entry probed on rc.3 is no tier entry
/// for a corpus that installs rc.9, however exactly its package matches.
/// The environment is an authored entry's `solidRuntime`, or the one an older
/// certified index named by root. One the index does not carry, or a pin the
/// corpus does not state, is no match: absence proves nothing.
export function bundleRuntimeMatches(bundle, index, pin) {
  const entries = bundle.solidRuntime ?? index?.environments?.[bundle.dependencyEnvironmentRoot];
  if (!Array.isArray(entries) || !pin) return false;
  for (const [name, pinnedBy] of Object.entries(RUNTIME_PINS)) {
    const recorded = entries.filter(entry => entry.name === name);
    const pinned = pin[pinnedBy];
    if (recorded.length === 0) continue;
    if (!pinned || recorded.some(entry => entry.version !== pinned)) return false;
  }
  return entries.some(entry => entry.name === "solid-js");
}

/// `name@version` -> host -> set of requested entrypoints the tier carries,
/// from an authored index (`entries`) or a certified one (`bundles`).
///
/// With `corpus`, only bundles whose recorded runtime is the one the corpus
/// entry installs count (`bundleRuntimeMatches`); the others are kept apart in
/// `tier.otherRuntime`, the same shape, so the report can say the package is in
/// the tier for another runtime rather than absent.
export function tierEntrypoints(index, corpus = null) {
  const tier = new Map();
  const otherRuntime = new Map();
  const pins = new Map((corpus?.packages ?? []).map(entry => [`${entry.package}@${entry.version}`, entry.solid ?? null]));
  for (const bundle of [...(index?.entries ?? []), ...(index?.bundles ?? [])]) {
    const key = `${bundle.packageName}@${bundle.packageVersion}`;
    const target = !corpus || bundleRuntimeMatches(bundle, index, pins.get(key)) ? tier : otherRuntime;
    const hosts = target.get(key) ?? new Map(HOSTS.map(host => [host, new Set()]));
    hosts.get(hostOfConditions(bundle.exportConditions)).add(bundle.requestedEntrypoint);
    target.set(key, hosts);
  }
  tier.otherRuntime = otherRuntime;
  return tier;
}

// ---------------------------------------------------------------------------
// Misuse ledger
// ---------------------------------------------------------------------------

/// Structural problems of the misuse ledger; a malformed case is never run.
export function ledgerProblems(ledger, corpus) {
  const problems = [];
  const ids = new Set();
  for (const [index, entry] of (ledger?.cases ?? []).entries()) {
    const label = entry?.id ?? `case ${index}`;
    if (!entry?.id || ids.has(entry.id)) problems.push(`${label}: a unique id is required`);
    ids.add(entry?.id);
    const pinned = corpus?.packages?.find(candidate => candidate.package === entry?.package);
    if (!pinned) problems.push(`${label}: ${entry?.package} is not in the checkpoint corpus`);
    else if (pinned.version !== entry.version) problems.push(`${label}: pins ${entry.version}, the corpus pins ${pinned.version}`);
    if (!MISUSE_RULES[entry?.class]) problems.push(`${label}: unknown misuse class ${entry?.class}`);
    if (typeof entry?.rule !== "string") problems.push(`${label}: the expected rule is required`);
    if (entry?.kind !== undefined && !FINDING_KINDS.includes(entry.kind)) problems.push(`${label}: unknown finding kind ${entry.kind}`);
    for (const field of ["misuse", "correct"]) if (typeof entry?.[field] !== "string" || entry[field].length === 0) problems.push(`${label}: ${field} code is required`);
    for (const host of entry?.hosts ?? []) if (!HOSTS.includes(host)) problems.push(`${label}: unknown host ${host}`);
    if (!Array.isArray(entry?.hosts) || entry.hosts.length === 0) problems.push(`${label}: hosts is required`);
  }
  return problems;
}

/// The finding kinds a case may expect of its misuse (CONTEXT.md). A case
/// states `kind: "uncertifiable"` when what the export does is a proof
/// obligation rather than a proven defect -- ADR 0161's may-register owner
/// requirement -- and a violation there is an overclaim.
export const FINDING_KINDS = ["violation", "uncertifiable"];

/// One host's verdict of one case from what TypeScript and the checker said.
export function misuseVerdict({ rule, kind = "violation", tsc, misuseFindings, correctFindings }) {
  if (tsc.misuse.length > 0 || tsc.correct.length > 0) {
    return { status: "tsc reports", detail: [...tsc.misuse, ...tsc.correct].map(code => `TS${code}`).join(", ") };
  }
  if (kind !== "violation" && misuseFindings.some(finding => finding.rule === rule && finding.kind === "violation")) {
    return { status: "misuse overclaims", detail: rule };
  }
  const expected = misuseFindings.filter(finding => finding.rule === rule && finding.kind === kind);
  if (expected.length === 0) {
    if (misuseFindings.some(finding => finding.rule === rule)) return { status: "misuse only uncertifiable", detail: rule };
    const others = [...new Set(misuseFindings.map(finding => finding.rule))];
    return { status: others.length ? "wrong finding" : "misuse silent", detail: others.join(", ") || null };
  }
  if (correctFindings.length > 0) {
    return { status: "correct use not clean", detail: [...new Set(correctFindings.map(finding => finding.rule))].join(", ") };
  }
  return { status: "reports correctly", detail: null };
}

async function evaluateMisuse({ ledger, corpus, only, packages = [], concurrency = defaultConcurrency(), checker, typefacts }) {
  const { oracleCompilerOptions } = await import("./tsc-oracle.mjs");
  const ts = loadTypeScript();
  const work = join(root, "rust/target/primitives-checkpoint/misuse");
  const entries = (ledger.cases ?? []).filter(entry => (!only.length || only.includes(entry.id)) && (!packages.length || packages.includes(entry.package)));
  const results = new Array(entries.length);
  let next = 0;
  // Cases are independent, so they run `concurrency` at a time; results keep
  // ledger order.
  const worker = async () => {
    while (next < entries.length) {
      const index = next++;
      results[index] = await evaluateMisuseCase({ entry: entries[index], corpus, work, ts, oracleCompilerOptions, checker, typefacts });
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, concurrency) }, worker));
  return { format: "solid-checker-primitives-misuse-results", version: 1, results };
}

function defaultConcurrency() {
  return Math.max(1, Math.min(8, Math.floor(availableParallelism() / 2)));
}

function runProcess(command, args, options) {
  return new Promise(resolvePromise => {
    execFile(command, args, { encoding: "utf8", maxBuffer: 256 * 1024 * 1024, ...options }, (error, stdout, stderr) =>
      resolvePromise({ status: error ? (typeof error.code === "number" ? error.code : 1) : 0, stdout, stderr: stderr || (error?.message ?? "") })
    );
  });
}

async function evaluateMisuseCase({ entry, corpus, work, ts, oracleCompilerOptions, checker, typefacts }) {
  const pinned = corpus.packages.find(candidate => candidate.package === entry.package);
  const dir = join(work, entry.id.replace(/[^A-Za-z0-9._-]/g, "_"));
  mkdirSync(dir, { recursive: true });
  // The head probe's install, as the benchmark makes it: the package, the
  // pinned solid-js, the matching @solidjs/web, and @solidjs/signals held at
  // the same release, so the tier's recorded environment can reproduce.
  const solid = pinned?.solid?.["solid-js"] ?? null;
  if (!solid) return { id: entry.id, package: entry.package, export: entry.export, hosts: {}, error: "the corpus entry pins no solid-js" };
  const dependencies = { [entry.package]: entry.version, "solid-js": solid, "@solidjs/web": pinned.solid["@solidjs/web"] ?? solid };
  const manifest = `${JSON.stringify({ name: "primitives-misuse-case", private: true, type: "module", overrides: { "@solidjs/signals": solid }, dependencies }, null, 2)}\n`;
  const manifestPath = join(dir, "package.json");
  const lockPath = join(dir, "bun.lock");
  // An unchanged manifest over a finished install needs no new install.
  const installed = existsSync(manifestPath) && readFileSync(manifestPath, "utf8") === manifest && existsSync(lockPath) && existsSync(join(dir, "node_modules"));
  if (!installed) {
    writeFileSync(manifestPath, manifest);
    const install = await runProcess("bun", ["install", "--no-summary"], { cwd: dir });
    if (install.status !== 0) return { id: entry.id, package: entry.package, export: entry.export, hosts: {}, error: `bun install failed: ${install.stderr.slice(0, 300)}` };
  }
  const options = oracleCompilerOptions("v2", true);
  // tsc's answer depends on the twin, the compiler options, the TypeScript
  // release and the installed typings (pinned by the lockfile), never on the
  // checker, so it is cached under a digest of exactly those.
  const cachePath = join(dir, "tsc-cache.json");
  const cache = existsSync(cachePath) ? JSON.parse(readFileSync(cachePath, "utf8")) : {};
  const tsc = {};
  for (const part of ["misuse", "correct"]) {
    const source = entry[part].endsWith("\n") ? entry[part] : `${entry[part]}\n`;
    writeFileSync(join(dir, `${part}.tsx`), source);
    writeFileSync(join(dir, `tsconfig.${part}.json`), `${JSON.stringify({ compilerOptions: options, files: [`${part}.tsx`] }, null, 2)}\n`);
    const key = createHash("sha256").update(JSON.stringify([source, options, ts.version, manifest, readFileSync(lockPath, "utf8")])).digest("hex");
    if (cache[part]?.key === key) {
      tsc[part] = cache[part].codes;
      continue;
    }
    const converted = ts.convertCompilerOptionsFromJson(options, dir);
    const program = ts.createProgram([join(dir, `${part}.tsx`)], converted.options);
    tsc[part] = ts
      .getPreEmitDiagnostics(program)
      .filter(diagnostic => diagnostic.category === ts.DiagnosticCategory.Error)
      .map(diagnostic => diagnostic.code);
    cache[part] = { key, codes: tsc[part] };
  }
  writeFileSync(cachePath, `${JSON.stringify(cache)}\n`);
  const runs = entry.hosts.flatMap(host => ["misuse", "correct"].map(part => ({ host, part })));
  const outputs = await Promise.all(
    runs.map(({ host, part }) =>
      runProcess(checker, ["--format", "json", "--project", join(dir, `tsconfig.${part}.json`), ...(host === "none" ? [] : ["--runtime-target", host])], {
        env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts },
      })
    )
  );
  const hosts = {};
  for (const host of entry.hosts) {
    const findings = {};
    for (const part of ["misuse", "correct"]) {
      const output = outputs[runs.findIndex(run => run.host === host && run.part === part)];
      let report;
      try {
        report = JSON.parse(output.stdout);
      } catch {
        return { id: entry.id, package: entry.package, export: entry.export, hosts: {}, error: `checker failed (${host}, ${part}, exit ${output.status}): ${output.stderr.slice(0, 300)}` };
      }
      findings[part] = (report.findings ?? []).map(finding => ({ rule: finding.rule, kind: finding.kind }));
    }
    hosts[host] = { ...misuseVerdict({ rule: entry.rule, kind: entry.kind, tsc, misuseFindings: findings.misuse, correctFindings: findings.correct }), findings };
  }
  return { id: entry.id, package: entry.package, entrypoint: entry.entrypoint ?? ".", export: entry.export, class: entry.class, rule: entry.rule, kind: entry.kind ?? "violation", tsc, hosts };
}

// ---------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------

const accepted = cause => ACCEPTED_UNPROVABLE.some(entry => entry.class === cause.class && entry.key === cause.key);

/// The agreed reason an export of a published-defect package is accounted for.
export const PUBLISHED_DEFECT_REASON = "published package defect";

/// A package's published defect when every host measured it and every one of
/// them read the defect from the bytes (`publishedDefectOf`, recomputed on
/// each run, never a name list): a package republished so that it loads in any
/// host has no defect there and so no defect here. A host not measured, or one
/// where the package loads, leaves the package undecided, so not accounted.
export function packageDefect(perHost) {
  const defects = perHost.map(([, measured]) => measured?.publishedDefect ?? null);
  return defects.every(Boolean) ? defects[0] : null;
}

/// Combines the per-host measurements, the tier and the misuse results into
/// the checkpoint: per package, the four criteria; per export, its status.
export function checkpoint({ hosts, tier, misuseResults = null, ledger = { cases: [] }, corpus }) {
  const byHost = new Map(hosts.map(result => [result.host, result]));
  const missingHosts = HOSTS.filter(host => !byHost.has(host));
  const packages = [];
  for (const entry of corpus.packages) {
    const perHost = HOSTS.map(host => [host, byHost.get(host)?.packages.find(item => item.package === entry.package) ?? null]);
    const tierHosts = tier.get(`${entry.package}@${entry.version}`) ?? new Map(HOSTS.map(host => [host, new Set()]));
    // Criterion 1, per host.
    const certification = {};
    for (const [host, measured] of perHost) {
      const surfaceEntrypoints = [...new Set((measured?.exports ?? []).map(item => item.entrypoint))].sort();
      const inTier = tierHosts.get(host) ?? new Set();
      const missingTier = surfaceEntrypoints.filter(entrypoint => !inTier.has(entrypoint));
      const otherRuntime = tier.otherRuntime?.get(`${entry.package}@${entry.version}`)?.get(host) ?? new Set();
      const tierOtherRuntime = missingTier.filter(entrypoint => otherRuntime.has(entrypoint));
      const certified = measured?.certification === "certified" && measured.certifiable && (measured.refusedEntrypoints ?? 0) === 0;
      certification[host] = {
        status: measured?.certification ?? "not measured",
        stage: measured?.stage ?? null,
        lane: measured?.lane ?? null,
        reason: measured?.reason ?? null,
        publishedDefect: measured?.publishedDefect ?? null,
        refusedEntrypoints: measured?.refusedEntrypoints ?? 0,
        tierEntrypoints: inTier.size,
        missingTier,
        tierOtherRuntime,
        met: Boolean(measured) && certified && missingTier.length === 0 && surfaceEntrypoints.length > 0
      };
    }
    const defect = packageDefect(perHost);
    // Criterion 2 and 3, per export (the union of every host's surface).
    const keys = new Set();
    for (const [, measured] of perHost) for (const item of measured?.exports ?? []) keys.add(`${item.entrypoint}\u0000${item.export}`);
    const exports = [];
    for (const key of [...keys].sort()) {
      const [entrypoint, name] = key.split("\u0000");
      const status = {};
      const misuse = new Map();
      for (const [host, measured] of perHost) {
        const item = measured?.exports.find(candidate => candidate.entrypoint === entrypoint && candidate.export === name) ?? null;
        status[host] = item
          ? { bucket: item.bucket, open: [...new Set(item.causes.map(cause => cause.domain).filter(Boolean))], causes: item.causes }
          : { bucket: measured ? "absent" : "not measured", open: [], causes: [] };
        for (const path of item?.misuse ?? []) {
          const held = misuse.get(path.class);
          if (!held || (held.source !== "accepted" && path.source === "accepted") || (held.source === "types" && path.source === "proposed")) misuse.set(path.class, path);
        }
      }
      const clean = HOSTS.every(host => status[host].bucket === "clean");
      const unprovable = !clean && HOSTS.every(host => status[host].bucket === "clean" || (status[host].causes.length > 0 && status[host].causes.every(accepted)));
      const inDefect = !clean && !unprovable && defect !== null;
      const cases = (ledger.cases ?? []).filter(item => item.package === entry.package && (item.entrypoint ?? ".") === entrypoint && item.export === name);
      const evaluated = cases.map(item => misuseResults?.results?.find(result => result.id === item.id) ?? null);
      const reporting = evaluated.filter(result => result && Object.values(result.hosts ?? {}).length > 0 && Object.values(result.hosts).every(host => host.status === "reports correctly"));
      // Types-only paths stand in only where no host's summary states a path.
      const stated = [...misuse.values()].filter(path => path.source !== "types");
      const paths = (stated.length ? stated : [...misuse.values()]).sort((left, right) => left.class.localeCompare(right.class));
      const fixture = paths.length === 0 ? "no misuse path" : cases.length === 0 ? "absent" : reporting.length > 0 ? "reports correctly" : evaluated.some(Boolean) ? "present, not reporting" : "present, not run";
      exports.push({
        entrypoint,
        export: name,
        certified: clean,
        accounted: clean || unprovable || inDefect,
        uncertifiableReason: inDefect ? PUBLISHED_DEFECT_REASON : null,
        status,
        misuse: paths,
        fixture,
        cases: cases.map(item => item.id)
      });
    }
    const criterion1 = HOSTS.every(host => certification[host].met);
    // A defect package whose surface could not be measured at all (nothing
    // loads, so no export is named) is accounted for by the defect alone.
    const criterion2 = defect !== null ? exports.every(item => item.accounted) : exports.length > 0 && exports.every(item => item.accounted);
    const criterion3 = exports.every(item => item.fixture === "no misuse path" || item.fixture === "reports correctly");
    packages.push({
      package: entry.package,
      version: entry.version,
      inMetricCorpus: entry.inMetricCorpus ?? false,
      inCensus: entry.inCensus ?? false,
      certification,
      publishedDefect: HOSTS.map(host => certification[host].publishedDefect).find(Boolean) ?? null,
      accountedAsDefect: defect !== null,
      exports,
      criteria: { certified: criterion1, accounted: criterion2, misuse: criterion3, appImport: null },
      atCheckpoint: criterion1 && criterion2 && criterion3
    });
  }
  const allExports = packages.flatMap(entry => entry.exports);
  const withPaths = allExports.filter(item => item.misuse.length > 0);
  const misuseByClass = {};
  for (const item of withPaths) for (const path of item.misuse) misuseByClass[path.class] = (misuseByClass[path.class] ?? 0) + 1;
  return {
    format: "solid-checker-primitives-checkpoint",
    version: 1,
    corpus: { measuredOn: corpus.measuredOn, packages: corpus.packages.length, withoutSolid2: (corpus.withoutSolid2 ?? []).length },
    missingHosts,
    runs: Object.fromEntries(hosts.map(result => [result.host, { durationMs: result.run?.durationMs ?? null, startedAt: result.run?.startedAt ?? null }])),
    progress: {
      packagesAtCheckpoint: packages.filter(entry => entry.atCheckpoint).length,
      packages: packages.length,
      criterion1: packages.filter(entry => entry.criteria.certified).length,
      criterion2: packages.filter(entry => entry.criteria.accounted).length,
      criterion3: packages.filter(entry => entry.criteria.misuse).length,
      publishedDefects: packages.filter(entry => entry.publishedDefect).map(entry => ({ package: entry.package, ...entry.publishedDefect })),
      exportsAccounted: allExports.filter(item => item.accounted).length,
      exportsAccountedAsDefect: allExports.filter(item => item.uncertifiableReason === PUBLISHED_DEFECT_REASON).length,
      exports: allExports.length,
      exportsCleanPerHost: Object.fromEntries(HOSTS.map(host => [host, allExports.filter(item => item.status[host]?.bucket === "clean").length])),
      exportsWithMisusePath: withPaths.length,
      misuseFromContract: withPaths.filter(item => item.misuse.some(path => path.source !== "types")).length,
      misuseByClass,
      misuseFixtures: {
        absent: withPaths.filter(item => item.fixture === "absent").length,
        present: withPaths.filter(item => item.fixture.startsWith("present")).length,
        reporting: withPaths.filter(item => item.fixture === "reports correctly").length
      }
    },
    walls: combineWalls(hosts),
    packages
  };
}

/// The per-host wall rankings joined by (class, key): exports blocked in each
/// host, and the distinct exports a wall blocks in any host.
export function combineWalls(hosts) {
  const walls = new Map();
  for (const result of hosts) {
    for (const measured of result.packages) {
      for (const item of measured.exports) {
        const attributed = item.causes.flatMap(cause => (cause.behind ? [cause, { class: "graph lane refused", key: cause.behind }] : [cause]));
        for (const cause of attributed) {
          const id = `${cause.class}\u0000${cause.key}`;
          const wall = walls.get(id) ?? { class: cause.class, key: cause.key, any: new Set(), hosts: {}, packages: new Set() };
          const exportKey = `${measured.package}\u0000${item.entrypoint}\u0000${item.export}`;
          wall.any.add(exportKey);
          wall.packages.add(measured.package);
          wall.hosts[result.host] ??= new Set();
          wall.hosts[result.host].add(exportKey);
          walls.set(id, wall);
        }
      }
    }
  }
  return [...walls.values()]
    .map(wall => ({
      class: wall.class,
      key: wall.key,
      exports: wall.any.size,
      packages: wall.packages.size,
      hosts: Object.fromEntries(HOSTS.map(host => [host, wall.hosts[host]?.size ?? 0])),
      onlyIn: HOSTS.filter(host => (wall.hosts[host]?.size ?? 0) > 0).length === 1 ? HOSTS.find(host => (wall.hosts[host]?.size ?? 0) > 0) : null
    }))
    .sort((left, right) => right.exports - left.exports || left.class.localeCompare(right.class) || left.key.localeCompare(right.key));
}

const mark = value => (value ? "yes" : "no");

export function renderMarkdown(result) {
  const lines = [];
  const progress = result.progress;
  lines.push("# @solid-primitives checkpoint");
  lines.push("");
  lines.push(`Corpus pinned ${result.corpus.measuredOn}: ${result.corpus.packages} packages with a Solid 2 release (${result.corpus.withoutSolid2} without).`);
  lines.push(`Runs: ${HOSTS.map(host => `${host} ${result.runs[host]?.durationMs == null ? "not measured" : `${Math.round(result.runs[host].durationMs / 1000)} s`}`).join(", ")}.`);
  if (result.missingHosts.length) lines.push(`**Hosts not measured:** ${result.missingHosts.join(", ")}.`);
  lines.push("");
  lines.push(`- packages at the checkpoint: **${progress.packagesAtCheckpoint} of ${progress.packages}**`);
  lines.push(`- criterion 1 (certified per host, in the tier): ${progress.criterion1}; criterion 2 (every export accounted): ${progress.criterion2}; criterion 3 (misuse reports): ${progress.criterion3}; criterion 4 (app-import metric): not measured`);
  lines.push(`- published-package defects (cannot load under the pinned runtime; owner ruling 2026-09-30: their exports are accounted for as uncertifiable, reason "${PUBLISHED_DEFECT_REASON}", nothing claimed, never clean): ${progress.publishedDefects.length ? progress.publishedDefects.map(entry => `\`${entry.package}\` (${entry.kind}: ${entry.detail})`).join(", ") : "none"}`);
  lines.push(`- exports accounted for: **${progress.exportsAccounted} of ${progress.exports}** (${progress.exportsAccountedAsDefect} as a published package defect); clean per host: ${HOSTS.map(host => `${host} ${progress.exportsCleanPerHost[host]}`).join(", ")}`);
  lines.push(`- exports with a misuse path: ${progress.exportsWithMisusePath} (${progress.misuseFromContract} from a contract claim) ${JSON.stringify(progress.misuseByClass)}; fixtures absent ${progress.misuseFixtures.absent}, present ${progress.misuseFixtures.present}, reporting ${progress.misuseFixtures.reporting}`);
  lines.push("");
  lines.push("## Per package");
  lines.push("");
  lines.push("| package | version | certified none / browser / node | tier none / browser / node | exports | clean (all hosts) | open | misuse paths | fixtures reporting | at checkpoint |");
  lines.push("| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const entry of result.packages) {
    const certification = HOSTS.map(host => (entry.certification[host].status === "certified" && entry.certification[host].refusedEntrypoints === 0 ? "yes" : entry.certification[host].status)).join(" / ");
    const tierCell = host => {
      const state = entry.certification[host];
      if (state.missingTier.length === 0 && state.tierEntrypoints > 0) return "yes";
      const other = (state.tierOtherRuntime ?? []).length ? `, other runtime (${state.tierOtherRuntime.length})` : "";
      return state.tierEntrypoints > 0 ? `partial (${state.tierEntrypoints}${other})` : other ? `no${other}` : "no";
    };
    const tier = HOSTS.map(tierCell).join(" / ");
    const clean = entry.exports.filter(item => item.certified).length;
    const paths = entry.exports.filter(item => item.misuse.length > 0).length;
    const reporting = entry.exports.filter(item => item.fixture === "reports correctly").length;
    lines.push(`| \`${entry.package}\` | ${entry.version} | ${certification} | ${tier} | ${entry.exports.length} | ${clean} | ${entry.exports.length - clean} | ${paths} | ${reporting} | ${mark(entry.atCheckpoint)} |`);
  }
  lines.push("");
  lines.push("## Blocking causes across the family");
  lines.push("");
  lines.push("| class | wall | exports (any host) | none | browser | node | packages | only in |");
  lines.push("| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const wall of result.walls.slice(0, 50)) {
    lines.push(`| ${wall.class} | ${wall.key.replace(/\|/g, "\\|")} | ${wall.exports} | ${wall.hosts.none} | ${wall.hosts.browser} | ${wall.hosts.node} | ${wall.packages} | ${wall.onlyIn ?? ""} |`);
  }
  lines.push("");
  lines.push("## Exports with a misuse path");
  lines.push("");
  lines.push("| package | export | misuse | rule | source | clean none / browser / node | fixture |");
  lines.push("| --- | --- | --- | --- | --- | --- | --- |");
  for (const entry of result.packages) {
    for (const item of entry.exports.filter(candidate => candidate.misuse.length > 0)) {
      const name = item.entrypoint === "." ? item.export : `${item.entrypoint} ${item.export}`;
      for (const path of item.misuse) {
        lines.push(`| \`${entry.package}\` | \`${name}\` | ${path.misuse} | ${path.code ? `${path.code} ${path.rule}` : "-"} | ${path.source} | ${HOSTS.map(host => (item.status[host].bucket === "clean" ? "yes" : item.status[host].open.join("+") || item.status[host].bucket)).join(" / ")} | ${item.fixture} |`);
      }
    }
  }
  return `${lines.join("\n")}\n`;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArguments(argv) {
  const options = { select: false, branch: true, printProbes: false, measure: null, misuse: false, cases: [], packages: [], concurrency: null, report: null, misuseResults: null, json: null, markdown: null, cleanRetained: false, corpus: CORPUS_PATH };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--select") options.select = true;
    else if (argument === "--no-branch") options.branch = false;
    else if (argument === "--print-probes") options.printProbes = true;
    else if (argument === "--measure") options.measure = resolve(argv[++index]);
    else if (argument === "--misuse") options.misuse = true;
    else if (argument === "--case") options.cases.push(argv[++index]);
    else if (argument === "--package") options.packages.push(argv[++index]);
    else if (argument === "--concurrency") options.concurrency = Number(argv[++index]);
    else if (argument === "--report") options.report = argv[++index].split(",").map(path => resolve(path));
    else if (argument === "--misuse-results") options.misuseResults = resolve(argv[++index]);
    else if (argument === "--json") options.json = resolve(argv[++index]);
    else if (argument === "--markdown") options.markdown = resolve(argv[++index]);
    else if (argument === "--clean-retained") options.cleanRetained = true;
    else if (argument === "--corpus") options.corpus = resolve(argv[++index]);
    else if (argument === "-h" || argument === "--help") {
      console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n\n")[1]);
      process.exit(0);
    } else fail(`unknown argument ${JSON.stringify(argument)}`);
  }
  return options;
}

const write = (path, text) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
};

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.select) {
    await select(options.corpus, { branch: options.branch });
    return;
  }
  const corpus = JSON.parse(readFileSync(options.corpus, "utf8"));
  if (options.printProbes) {
    const problems = checkpointCorpusProblems(corpus, JSON.parse(readFileSync(MANIFEST_PATH, "utf8")));
    if (problems.length) fail(`the manifest no longer agrees with the pinned corpus:\n  ${problems.join("\n  ")}\nre-pin with --select`);
    console.log(corpus.packages.map(entry => entry.probe).join("\n"));
    return;
  }
  if (options.measure) {
    const run = JSON.parse(readFileSync(options.measure, "utf8"));
    const rows = new Map();
    const typePaths = new Map();
    for (const result of run.results ?? []) {
      if (!corpus.packages.some(entry => entry.probe === result.probeId)) continue;
      const row = readRetainedRow(result);
      if (!row && result.retainedArtifacts?.outputDir) fail(`${result.probeId}: ${result.retainedArtifacts.outputDir} is gone; re-run make primitives-checkpoint`);
      rows.set(result.probeId, row);
      typePaths.set(result.probeId, typeMisusePaths(row?.generated ?? null, result.retainedArtifacts?.projectDir ?? null));
    }
    const measured = measureHost({ run, corpus, rows, typePaths });
    if (options.json) write(options.json, `${JSON.stringify(measured, null, 2)}\n`);
    console.log(`${measured.host}: ${measured.packages.length} packages, ${measured.headline.cleanExports} of ${measured.headline.exports} exports clean`);
    if (options.cleanRetained) {
      for (const result of run.results ?? []) {
        for (const directory of [result.retainedArtifacts?.projectDir, result.retainedArtifacts?.outputDir]) {
          if (directory && /\/solid-checker-ecosystem-[^/]+$/.test(directory) && existsSync(directory) && statSync(directory).isDirectory()) {
            rmSync(directory, { recursive: true, force: true });
          }
        }
      }
    }
    if (measured.missingProbes.length) fail(`the run is missing ${measured.missingProbes.length} corpus probe(s)`);
    return;
  }
  const ledger = JSON.parse(readFileSync(MISUSE_LEDGER_PATH, "utf8"));
  const problems = ledgerProblems(ledger, corpus);
  if (problems.length) fail(`the misuse ledger is malformed:\n  ${problems.join("\n  ")}`);
  if (options.misuse) {
    const checker = process.env.SOLID_CHECKER_NATIVE_BIN;
    const typefacts = process.env.SOLID_TYPEFACTS_BIN;
    if (!checker || !existsSync(checker) || !typefacts || !existsSync(typefacts)) {
      fail("--misuse needs SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN pointing at real files");
    }
    const results = await evaluateMisuse({ ledger, corpus, only: options.cases, packages: options.packages, ...(options.concurrency ? { concurrency: options.concurrency } : {}), checker, typefacts });
    if (options.json) write(options.json, `${JSON.stringify(results, null, 2)}\n`);
    for (const result of results.results) console.log(`${result.id}: ${result.error ?? Object.entries(result.hosts).map(([host, verdict]) => `${host} ${verdict.status}`).join(", ")}`);
    return;
  }
  if (!options.report) fail("one of --select, --print-probes, --measure, --misuse or --report is required");
  const hosts = options.report.map(path => JSON.parse(readFileSync(path, "utf8")));
  const tier = tierEntrypoints(JSON.parse(readFileSync(TIER_INDEX_PATH, "utf8")), corpus);
  const misuseResults = options.misuseResults && existsSync(options.misuseResults) ? JSON.parse(readFileSync(options.misuseResults, "utf8")) : null;
  const result = checkpoint({ hosts, tier, misuseResults, ledger, corpus });
  const markdown = renderMarkdown(result);
  if (options.json) write(options.json, `${JSON.stringify(result, null, 2)}\n`);
  if (options.markdown) write(options.markdown, markdown);
  console.log(markdown.split("\n## Per package")[0]);
}

if (import.meta.main) await main();
