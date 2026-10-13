// Derive assumptions from the consumer's installed bytes. No accepted contract
// or cross-runtime equivalence is implied by a local cache hit.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { authenticateModel, closurePins, hash, nativeRuntimeRoots, packageRoot, read } from "./catalog.mjs";
import { callableValue, literal, objectValue, SourceExtractor, runtimeEntry, unknownValue } from "./source-extractor.mjs";
import { ts } from "./lower.mjs";

const runtimeNames = ["solid-js", "@solidjs/signals", "@solidjs/web"];
const extractorSha256 = hash(readFileSync(new URL("./source-extractor.mjs", import.meta.url)));
export function installedCatalog(project, requests, host = "browser") {
  assert(["browser", "node"].includes(host));
  const started = performance.now(), models = [], packages = [];
  const grouped = new Map();
  for (const request of requests) {
    const names = grouped.get(request.package) ?? new Set();
    for (const name of request.exports) names.add(name);
    grouped.set(request.package, names);
  }
  for (const [name, requested] of grouped) {
    const item = { package: name, exports: [...requested].sort(), observations: [], error: null }; packages.push(item);
    try {
      // Keep this experiment within the reviewed native vocabulary. Deriving
      // fresh bytes is not permission to interpret an older RC as rc.9.
      for (const root of nativeRuntimeRoots(project)) {
        const runtime = read(join(root, "package.json"));
        assert.equal(runtime.version, "2.0.0-rc.9", `Runtime mismatch: ${runtime.name}`);
      }
      const root = packageRoot(project, name), manifest = read(join(root, "package.json")), pins = closurePins(root);
      for (const pin of pins.filter(pin => runtimeNames.includes(pin.package)))
        assert.equal(pin.version, "2.0.0-rc.9", `Nested runtime mismatch: ${pin.package}`);
      const engine = new SourceExtractor(host), entry = runtimeEntry(join(root, "package.json"), name, host);
      const exports = {};
      for (const exported of requested) {
        const observation = engine.extract(entry, exported); item.observations.push(observation);
        if (Object.keys(observation.behavior).length) exports[exported] = { [host]: observation.behavior };
      }
      assert.deepEqual(closurePins(root), pins, "Package inputs changed during extraction");
      models.push({ package: name, version: manifest.version, basis: "source-extracted-assumption", pins, exports,
        requestedExports: [...requested].sort(), runtimeEntries: { [host]: entry },
        sourceReferences: [...engine.modules.values()].map(module => ({ path: module.path, sha256: hash(module.source.text) })) });
      item.version = manifest.version;
    } catch (error) { item.error = error.message; }
  }
  return { format: "solid-checker-reviewed-model-experiment", version: 1, authority: false, runtime: "2.0.0-rc.9",
    basis: "source-extracted-assumption", extractorSha256, models, packages, durationMs: performance.now() - started };
}

function unwrap(node) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  return node;
}
// Interpret only syntax whose value cannot change before the package call.
// Identifier types alone cannot establish a runtime branch or immutable value.
export function argumentValue(node) {
  node = unwrap(node);
  if (ts.isNumericLiteral(node)) return literal(Number(node.text));
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return literal(node.text);
  if (node.kind === ts.SyntaxKind.TrueKeyword || node.kind === ts.SyntaxKind.FalseKeyword) return literal(node.kind === ts.SyntaxKind.TrueKeyword);
  if (node.kind === ts.SyntaxKind.NullKeyword) return literal(null);
  if (ts.isArrowFunction(node) || ts.isFunctionExpression(node)) return callableValue();
  if (ts.isObjectLiteralExpression(node)) {
    const fields = {};
    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) return unknownValue("opaque consumer object");
      fields[property.name.text] = argumentValue(property.initializer);
    }
    // Constructing the object may execute an earlier call/getter that changes
    // subsequent fields. Only closed literal/callable trees establish values.
    if (Object.values(fields).some(value => value.kind === "unknown")) return unknownValue("opaque consumer object field");
    return objectValue(fields);
  }
  return unknownValue("consumer argument is not a closed literal or inline function");
}

export function demandSpecializer(catalog, project) {
  assert.equal(catalog.extractorSha256, extractorSha256, "Extractor inputs changed");
  const engines = new Map(), cache = new Map();
  for (const model of catalog.models) authenticateModel(model, project);
  const stats = { extractions: 0, cacheHits: 0 };
  const specialize = ({ model, name, node, host }) => {
    const entry = model.runtimeEntries[host];
    if (!entry) return { behavior: {}, gaps: ["runtime entry is not modeled"] };
    const args = node.arguments.map(argumentValue);
    const profile = args.map(value => value.kind === "literal" && value.value === undefined ? { kind: "literal", type: "undefined" } : value);
    const key = JSON.stringify([model.package, model.pins, host, name, profile]);
    if (cache.has(key)) { stats.cacheHits++; return cache.get(key); }
    const engineKey = `${model.package}:${host}`;
    if (!engines.has(engineKey)) engines.set(engineKey, new SourceExtractor(host));
    const observation = engines.get(engineKey).extract(entry, name, args);
    const result = { ...observation, argumentProfile: profile, extractorSha256 };
    cache.set(key, result); stats.extractions++; return result;
  };
  specialize.stats = stats;
  return specialize;
}
