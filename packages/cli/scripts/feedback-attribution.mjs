import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";

// Runtime frames are attributed to configured original source only through
// the served module's own source map. Every frame keeps the outcome that
// stopped it, so an unmapped record says why rather than only that it failed.
// Package ownership is the nearest installed package.json; it names where the
// code lives, never who is responsible for the read.

const installed = directory => directory.split(sep).includes("node_modules");

export function createPackageLookup() {
  const cache = new Map();
  return function packageOf(path) {
    let directory = dirname(path);
    const visited = [];
    while (dirname(directory) !== directory) {
      if (cache.has(directory)) break;
      visited.push(directory);
      const metadata = join(directory, "package.json");
      if (existsSync(metadata)) {
        let pkg = null;
        try { pkg = JSON.parse(readFileSync(metadata, "utf8")); } catch { pkg = null; }
        if (typeof pkg?.name === "string") {
          cache.set(directory, { name: pkg.name, version: typeof pkg.version === "string" ? pkg.version : null,
            directory, installed: installed(directory) });
          break;
        }
      }
      directory = dirname(directory);
    }
    const found = cache.get(directory) ?? null;
    for (const row of visited) cache.set(row, found);
    return found;
  };
}

export function createFrameAttributor({ origin, root, sourceFiles, collectorPaths = [], mapFor, originalPositionFor,
  packageOf = createPackageLookup() }) {
  const collector = new Set(collectorPaths);
  return async function attribute(frame) {
    let url; try { url = new URL(frame.path); } catch { return { outcome: "not-a-url" }; }
    if (url.origin !== origin) return { outcome: "foreign-origin" };
    if (url.pathname.startsWith("/@id/")) return { outcome: "virtual-module" };
    const served = url.pathname.startsWith("/@fs/") ? decodeURIComponent(url.pathname.slice(4)) : join(root, decodeURIComponent(url.pathname));
    if (!existsSync(served)) return { outcome: "served-path-missing", served };
    const file = realpathSync(served);
    if (collector.has(file)) return { outcome: "collector", served: file };
    const owner = packageOf(file), described = { served: file, package: owner && { name: owner.name, version: owner.version, installed: owner.installed } };
    const map = await mapFor(url.pathname + url.search);
    if (!map) return { outcome: "no-source-map", ...described };
    const position = originalPositionFor(map, { line: frame.line, column: frame.column - 1 });
    if (!position.source || !position.line) return { outcome: "no-original-position", ...described };
    const path = resolve(dirname(served), position.source), original = { path, line: position.line, column: position.column + 1 };
    const source = sourceFiles.get(path);
    if (!source) return { outcome: "outside-configured-sources", ...described, original };
    const lines = source.text.split("\n");
    if (position.line > lines.length || position.column >= lines[position.line - 1].length)
      return { outcome: "position-out-of-range", ...described, original };
    return { outcome: "mapped", ...described, original, source };
  };
}

const reasons = {
  "application-frame-unmapped": "An application-owned frame did not map to a configured original source; application attribution was lost.",
  "package-frames-only": "Every captured frame belongs to an installed package; no application frame was on the synchronous stack. Responsibility remains open.",
  "stack-incomplete": "The captured stack is or may be incomplete and holds no application frame; attribution remains open.",
  "no-attributable-frame": "No captured frame was served from this application's server."
};

// `stackTruncated` is true when the stack reached its capture limit and null
// when its completeness is unknown. Only a complete stack can show that no
// application frame was present.
export function classifyUnmappedFrames(frames, attributions, { stackTruncated = null, readerPackage = "@solidjs/signals" } = {}) {
  const rows = frames.map((frame, index) => ({ ...frame, ...attributions[index], source: undefined }));
  const relevant = rows.filter(row => row.served && row.outcome !== "collector");
  let attribution;
  if (relevant.some(row => !row.package?.installed)) attribution = "application-frame-unmapped";
  else if (relevant.length && stackTruncated === false) attribution = "package-frames-only";
  else if (relevant.length) attribution = "stack-incomplete";
  else attribution = "no-attributable-frame";
  const packages = [];
  for (const row of relevant) {
    const pkg = row.package;
    if (pkg?.installed && !packages.some(item => item.name === pkg.name && item.version === pkg.version))
      packages.push({ name: pkg.name, version: pkg.version });
  }
  const first = relevant.find(row => row.package?.installed && row.package.name !== readerPackage) ?? null;
  return { attribution, reason: reasons[attribution], stackTruncated, packages,
    firstPackageFrame: first && { package: { name: first.package.name, version: first.package.version },
      served: first.served, line: first.line, column: first.column, original: first.original ?? null },
    frames: rows.map(row => ({ path: row.path, line: row.line, column: row.column, outcome: row.outcome,
      ...(row.package ? { package: `${row.package.name}@${row.package.version}`, installed: row.package.installed } : {}),
      ...(row.original ? { original: row.original } : {}) })) };
}

export function summarizeUnmapped(unmapped) {
  const byAttribution = {};
  for (const row of unmapped) byAttribution[row.attribution] = (byAttribution[row.attribution] ?? 0) + 1;
  return { total: unmapped.length, byAttribution };
}

const span = value => `${value.start}:${value.end}`;
const accepted = new Set(["return-expression", "local-initializer", "distinct-branch"]);

// Joins native candidate models to what the instrumented run entered. A
// candidate is a function carrying a derived origin or an operation with an
// accepted result relationship. Entry counts are synchronous; an async body's
// settlement is not observed here.
export function summarizeCandidateScopes(models, { instrumented, scopes, limit = 64, lineOf = () => null }) {
  const entered = new Map(scopes.rows.map(row => [`${row.kind}:${row.path}:${span(row.span)}`, row]));
  const totals = {
    files: { modeled: models.length, loaded: 0 },
    derivedOrigins: { modeled: 0, instrumented: 0, entered: 0 },
    resultOperations: { modeled: 0, instrumented: 0, entered: 0, returned: 0, threw: 0 }
  };
  const notEntered = [];
  let notEnteredDropped = 0;
  const missing = row => { if (notEntered.length < limit) notEntered.push(row); else notEnteredDropped++; };
  for (const model of models) {
    const file = instrumented.get(model.path);
    if (file) totals.files.loaded++;
    const candidates = [
      ...model.functions.filter(row => row.derivedOrigin).map(row => ({ kind: "function", total: totals.derivedOrigins, row })),
      ...model.operations.filter(row => accepted.has(row.resultRelevance)).map(row => ({ kind: "operation", total: totals.resultOperations, row }))
    ];
    for (const { kind, total, row } of candidates) {
      total.modeled++;
      const key = span(row.span), base = { kind, path: model.path, span: row.span, line: lineOf(model.path, row.span.start) };
      if (!file) { missing({ ...base, status: "file-not-loaded" }); continue; }
      if (!(kind === "function" ? file.functions : file.operations).has(key)) { missing({ ...base, status: "not-instrumented" }); continue; }
      total.instrumented++;
      const count = entered.get(`${kind}:${model.path}:${key}`);
      if (!count?.entered) { missing({ ...base, status: "not-entered" }); continue; }
      total.entered++;
      if (kind === "operation") { total.returned += count.returned > 0; total.threw += count.threw > 0; }
    }
  }
  return { ...totals, notEntered, notEnteredDropped, scopeRecordsDropped: scopes.dropped,
    asyncSettlement: "unobserved", complete: false };
}

// Solid's own runtime packages: a diagnostic is emitted from inside them, so
// their frames say nothing about which code performed the operation.
export const SOLID_RUNTIME_PACKAGES = new Set(["@solidjs/signals", "solid-js", "@solidjs/web"]);

// The code that performed a diagnosed operation: the innermost frame that is
// neither Solid's runtime, the collector nor an unserved frame. "application"
// is code the project owns; "package" is an installed package called from it.
export function operationFrame(attributions) {
  for (const row of attributions) {
    if (!row.served || row.outcome === "collector") continue;
    if (row.package?.installed && SOLID_RUNTIME_PACKAGES.has(row.package.name)) continue;
    return row.package?.installed
      ? { in: "package", package: { name: row.package.name, version: row.package.version } }
      : { in: "application" };
  }
  return { in: "unknown" };
}
