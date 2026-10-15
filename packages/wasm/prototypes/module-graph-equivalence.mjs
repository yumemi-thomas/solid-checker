// PROTOTYPE EVIDENCE: run the canonical Type Facts producer's module graph
// natively and as a WASI reactor over the same frozen project bytes, at the
// same absolute paths, and compare the raw responses. Manual evidence only:
// not a maintained provider, not a gate, not certification authority. See
// module-graph-equivalence.md beside this file.
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { WASI } from "@napi-rs/wasm-runtime";
import { memfs } from "@napi-rs/wasm-runtime/fs";

// @tybys/wasm-util logs expected WASI errno probes in development mode.
process.env.NODE_ENV ||= "production";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");
const reactorPackage = "./apps/solid-typefacts/prototypes/typefacts-reactor";
const fixture = resolve(repositoryRoot, reactorPackage, "testdata/forms");
const scratchRequest = process.env.MODULE_GRAPH_SCRATCH || "/tmp/sc-scratch.noindex/p5-canonical-module-graph";
mkdirSync(scratchRequest, { recursive: true });
// Realpath first: /tmp is a symlink on macOS, and the resolver reports
// realpaths, so both sides must see the project at its real path.
const scratch = realpathSync(scratchRequest);
if (!relative(repositoryRoot, scratch).startsWith("..")) {
  // The whole apps/solid-typefacts tree is Type Facts source identity.
  throw new Error(`scratch ${scratch} must lie outside the repository`);
}
const out = join(scratch, `witness-${new Date().toISOString().replace(/[:.]/g, "-")}`);
mkdirSync(join(out, "bin"), { recursive: true });

const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const failures = [];
const records = [];
function check(condition, message) {
  if (!condition) failures.push(message);
  return condition;
}

// --- build both images from the same package source -----------------------
function go(args, env = {}) {
  const result = spawnSync("go", args, {
    cwd: repositoryRoot,
    env: { ...process.env, GOPROXY: "off", GOTOOLCHAIN: "local", ...env },
    encoding: "utf8",
    maxBuffer: 1 << 26,
  });
  if (result.status !== 0) {
    throw new Error(`go ${args.join(" ")} failed (${result.status}):\n${result.stderr}`);
  }
  return result.stdout.trim();
}
const nativeBin = join(out, "bin", "reactor-native");
const wasmPath = join(out, "bin", "reactor.wasm");
go(["build", "-trimpath", "-o", nativeBin, reactorPackage]);
go(
  ["build", "-trimpath", "-buildmode=c-shared", "-ldflags=-s -w", "-o", wasmPath, reactorPackage],
  { GOOS: "wasip1", GOARCH: "wasm" },
);
const wasmBytes = readFileSync(wasmPath);
const wasmModule = await WebAssembly.compile(wasmBytes);
const images = {
  goVersion: go(["version"]),
  native: { path: nativeBin, sha256: sha256(readFileSync(nativeBin)) },
  wasm: { path: wasmPath, sha256: sha256(wasmBytes) },
};

// --- frozen project trees -------------------------------------------------
// Each case gets a fresh copy, mutated before anything reads it, then frozen:
// the bytes read here are the bytes mounted into memfs, and the disk tree is
// re-hashed after the native run to prove it did not move.
function snapshot(root) {
  const entries = new Map();
  const walk = directory => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      const stat = lstatSync(path);
      if (stat.isSymbolicLink()) entries.set(path, { link: readlinkSync(path) });
      else if (stat.isDirectory()) {
        entries.set(path, { directory: true });
        walk(path);
      } else entries.set(path, { bytes: readFileSync(path) });
    }
  };
  walk(root);
  return entries;
}
function manifest(entries) {
  return Object.fromEntries([...entries].map(([path, entry]) => [
    path,
    entry.bytes ? sha256(entry.bytes) : entry.link ? `link:${entry.link}` : "dir",
  ]));
}
function freeze(name, mutate = () => {}) {
  const caseRoot = join(out, "runs", name);
  const root = join(caseRoot, "forms");
  cpSync(fixture, root, { recursive: true });
  mutate(root);
  const entries = snapshot(root);
  return { name, caseRoot, root, entries, manifest: manifest(entries) };
}

// --- the two runs ---------------------------------------------------------
function runNative(tree, requestBytes) {
  const result = spawnSync(nativeBin, [], { input: requestBytes, maxBuffer: 1 << 30 });
  const stdout = result.stdout;
  writeFileSync(join(tree.caseRoot, "native.stdout"), stdout);
  if (stdout.length === 0 || stdout[stdout.length - 1] !== 0x0a) {
    throw new Error(`${tree.name}: native output lacks its framing newline`);
  }
  const after = manifest(snapshot(tree.root));
  check(JSON.stringify(after) === JSON.stringify(tree.manifest), `${tree.name}: disk tree moved during the native run`);
  return { status: result.status, bytes: stdout.subarray(0, stdout.length - 1) };
}

function runWasi(tree, requestBytes, omit = new Set()) {
  const { fs, vol } = memfs({}, "/");
  vol.mkdirSync(tree.root, { recursive: true });
  for (const [path, entry] of tree.entries) {
    if (omit.has(path)) continue;
    if (entry.directory) vol.mkdirSync(path, { recursive: true });
    else if (entry.link) vol.symlinkSync(entry.link, path);
    else vol.writeFileSync(path, entry.bytes);
  }
  const wasi = new WASI({ version: "preview1", fs, preopens: { "/": "/" } });
  const instance = new WebAssembly.Instance(wasmModule, wasi.getImportObject());
  wasi.initialize(instance);
  const exports = instance.exports;
  const pointer = exports.allocate_input(requestBytes.length);
  new Uint8Array(exports.memory.buffer, pointer, requestBytes.length).set(requestBytes);
  const status = exports.run_typefacts();
  const bytes = Buffer.from(new Uint8Array(
    exports.memory.buffer,
    exports.output_pointer(),
    exports.output_length(),
  ));
  writeFileSync(join(tree.caseRoot, "wasi.out"), bytes);
  return { status, bytes };
}

// rootFields locates each top-level member of one JSON object by byte range,
// without parsing or re-encoding any value. Structural bytes are ASCII and
// UTF-8 continuation bytes never are, so scanning bytes is exact.
function rootFields(bytes) {
  let index = 0;
  const space = () => {
    while ([0x20, 0x0a, 0x0d, 0x09].includes(bytes[index])) index++;
  };
  const expect = byte => {
    if (bytes[index] !== byte) throw new Error(`malformed JSON at byte ${index}`);
    index++;
  };
  const string = () => {
    const start = index;
    expect(0x22);
    while (bytes[index] !== 0x22) {
      if (index >= bytes.length) throw new Error("unterminated string");
      index += bytes[index] === 0x5c ? 2 : 1;
    }
    index++;
    return [start, index];
  };
  const value = () => {
    const start = index;
    if (bytes[index] === 0x22) return string();
    if (bytes[index] === 0x7b || bytes[index] === 0x5b) {
      let depth = 0;
      do {
        const byte = bytes[index];
        if (byte === 0x22) {
          string();
          continue;
        }
        if (byte === 0x7b || byte === 0x5b) depth++;
        else if (byte === 0x7d || byte === 0x5d) depth--;
        index++;
        if (index > bytes.length) throw new Error("unterminated container");
      } while (depth > 0);
      return [start, index];
    }
    while (index < bytes.length && ![0x2c, 0x7d, 0x20, 0x0a, 0x0d, 0x09].includes(bytes[index])) index++;
    return [start, index];
  };
  const fields = new Map();
  space();
  expect(0x7b);
  space();
  if (bytes[index] === 0x7d) index++;
  else {
    for (;;) {
      space();
      const [keyStart, keyEnd] = string();
      const key = JSON.parse(bytes.subarray(keyStart, keyEnd).toString("utf8"));
      space();
      expect(0x3a);
      space();
      const range = value();
      if (fields.has(key)) throw new Error(`duplicate root key ${key}`);
      fields.set(key, bytes.subarray(...range));
      space();
      if (bytes[index] === 0x2c) {
        index++;
        continue;
      }
      expect(0x7d);
      break;
    }
  }
  space();
  if (index !== bytes.length) throw new Error("trailing bytes after the root object");
  return fields;
}

function compare(tree, label, native, wasi) {
  const nativeFields = rootFields(native.bytes);
  const wasiFields = rootFields(wasi.bytes);
  const field = (fields, key) => fields.get(key);
  const same = (key) => {
    const left = field(nativeFields, key);
    const right = field(wasiFields, key);
    if (left === undefined || right === undefined) return left === right;
    return left.equals(right);
  };
  const record = {
    case: tree.name,
    label,
    status: { native: native.status, wasi: wasi.status },
    sha256: { native: sha256(native.bytes), wasi: sha256(wasi.bytes) },
    bytes: { native: native.bytes.length, wasi: wasi.bytes.length },
    rootKeys: { native: [...nativeFields.keys()], wasi: [...wasiFields.keys()] },
    wholeResponseEqual: native.bytes.equals(wasi.bytes),
    moduleGraphEqual: same("moduleGraph"),
    tableEqual: same("table"),
  };
  records.push(record);
  return { record, nativeFields, wasiFields };
}

// --- expectations from the frozen bytes -------------------------------------
function spanOf(tree, file, needle, occurrence) {
  const bytes = tree.entries.get(join(tree.root, file)).bytes;
  let offset = 0;
  for (let seen = 0; ; seen++) {
    const next = bytes.indexOf(needle, offset);
    if (next < 0) throw new Error(`${file} lacks occurrence ${occurrence} of ${needle}`);
    if (seen === occurrence) return [next, next + Buffer.byteLength(needle)];
    offset = next + Buffer.byteLength(needle);
  }
}
const sources = [
  "src/alias-use.ts", "src/alias/helper.ts", "src/ambient.d.ts", "src/dynamic.ts",
  "src/equals.ts", "src/forms.ts", "src/jsdoc.js", "src/local-bound.js",
  "src/param-bound.js", "src/ts-require.ts", "src/unbound.js", "src/unresolved.ts",
];
function positiveRequest(tree) {
  const [start, end] = spanOf(tree, "src/alias-use.ts", "aliased", 0);
  return {
    projectId: join(tree.root, "tsconfig.json"),
    // A freshly opened program is generation 1; the closure refuses any other.
    generation: 1,
    demands: [{ location: { path: join(tree.root, "src/alias-use.ts"), startByte: start, endByte: end }, symbol: true }],
    moduleGraph: {
      imports: true,
      packages: true,
      importPaths: [...sources.map(file => join(tree.root, file)), join(tree.root, "src/absent.ts")],
    },
  };
}
function expectedRows(tree, omitted = new Set()) {
  const at = (file, needle, occurrence, text, resolution) => {
    const [startByte, endByte] = spanOf(tree, file, needle, occurrence);
    return { path: join(tree.root, file), startByte, endByte, text, resolution };
  };
  const pkg = (file, occurrence = 0) => at(file, '"pkg"', occurrence, "pkg", "nodeModules");
  return [
    at("src/alias-use.ts", '"@alias/helper"', 0, "@alias/helper", "nonRelative"),
    pkg("src/ambient.d.ts"),
    pkg("src/dynamic.ts"),
    pkg("src/equals.ts"),
    pkg("src/forms.ts", 0),
    pkg("src/forms.ts", 1),
    pkg("src/forms.ts", 2),
    pkg("src/jsdoc.js"),
    pkg("src/local-bound.js"),
    pkg("src/param-bound.js"),
    pkg("src/unbound.js"),
    at("src/unbound.js", '""', 0, "", "unresolved"),
    at("src/unresolved.ts", '"missing-package"', 0, "missing-package", "unresolved"),
  ].filter(row => !omitted.has(row.path));
}
// assertAnswered requires the table to answer the demand with exactly one
// entity at its exact location, carrying a resolved, nonempty symbol.
function assertAnswered(prefix, table, demand) {
  const matches = (table.entities ?? []).filter(entity =>
    entity.location.path === demand.location.path &&
    entity.location.startByte === demand.location.startByte &&
    entity.location.endByte === demand.location.endByte);
  check(
    demand.location.startByte === 53 && demand.location.endByte === 60 &&
      matches.length === 1 && typeof matches[0].symbol === "string" && matches[0].symbol !== "" &&
      matches[0].symbolUnresolved !== true,
    `${prefix}: demand ${JSON.stringify(demand.location)} answered ${JSON.stringify(matches)}`,
  );
  return matches[0];
}
function assertPositive(tree, label, response, request, omitted = new Set()) {
  const parsed = JSON.parse(response.bytes.toString("utf8"));
  const prefix = `${tree.name}/${label}`;
  if (!check(response.status === 0 && parsed.ok === true, `${prefix}: not ok (${parsed.error})`)) return parsed;
  const table = parsed.table;
  assertAnswered(prefix, table, request.demands[0]);
  const digests = new Map(table.sources.map(source => [source.path, source.sha256]));
  for (const [path, entry] of tree.entries) {
    if (!entry.bytes || path.endsWith(".d.ts") || path.endsWith(".json")) continue;
    if (omitted.has(path)) {
      check(!digests.has(path), `${prefix}: omitted ${path} still has a digest`);
      continue;
    }
    check(digests.get(path) === `sha256:${sha256(entry.bytes)}`, `${prefix}: digest of ${path} is ${digests.get(path)}`);
  }
  const graph = parsed.moduleGraph;
  const modules = new Map(graph.modules.map(module => [module.path, module]));
  check(graph.modules.some(module => module.path.startsWith("bundled:") && module.declarationFile), `${prefix}: no bundled library`);
  for (const file of ["node_modules/pkg/index.js", "node_modules/pkg/index.d.ts", ...sources]) {
    const path = join(tree.root, file);
    const module = modules.get(path);
    if (omitted.has(path)) {
      check(module === undefined, `${prefix}: omitted ${file} is still a module`);
      continue;
    }
    check(module !== undefined, `${prefix}: inventory lacks ${file}`);
    check(module && Boolean(module.declarationFile) === file.endsWith(".d.ts"), `${prefix}: ${file} declarationFile`);
  }
  const unknown = [...omitted, join(tree.root, "src/absent.ts")].sort();
  check(JSON.stringify(graph.unknownImportPaths) === JSON.stringify(unknown), `${prefix}: unknownImportPaths ${JSON.stringify(graph.unknownImportPaths)}`);
  const projected = graph.imports.map(fact => ({
    path: fact.specifier.path,
    startByte: fact.specifier.startByte,
    endByte: fact.specifier.endByte,
    text: fact.text,
    resolution: fact.resolution,
  }));
  check(JSON.stringify(projected) === JSON.stringify(expectedRows(tree, omitted)), `${prefix}: rows ${JSON.stringify(projected)}`);
  for (const fact of graph.imports) {
    if (fact.text === "pkg") {
      check(
        fact.resolvedPath === join(tree.root, "node_modules/pkg/index.d.ts") && fact.extension === ".d.ts" &&
          JSON.stringify(fact.package) === JSON.stringify({
            manifestPath: join(tree.root, "node_modules/pkg/package.json"), name: "pkg", version: "1.2.3",
          }) &&
          fact.resolverPackage?.name === "pkg" && fact.resolverPackage?.version === "1.2.3",
        `${prefix}: npm row ${JSON.stringify(fact)}`,
      );
    } else if (fact.text === "@alias/helper") {
      check(
        fact.resolvedPath === join(tree.root, "src/alias/helper.ts") && fact.pathsPattern === "@alias/*",
        `${prefix}: alias row ${JSON.stringify(fact)}`,
      );
    } else {
      check(fact.resolvedPath === undefined && fact.package === undefined, `${prefix}: unresolved row ${JSON.stringify(fact)}`);
    }
  }
  return parsed;
}
const encodeRequest = (tree, request) => {
  const bytes = Buffer.from(JSON.stringify(request));
  writeFileSync(join(tree.caseRoot, "request.json"), bytes);
  return bytes;
};

// --- cases ----------------------------------------------------------------
// 1. Positive: semantic demand plus the full graph demand.
const positive = freeze("positive");
const positiveRequestValue = positiveRequest(positive);
const positiveBytes = encodeRequest(positive, positiveRequestValue);
const positiveNative = runNative(positive, positiveBytes);
const positiveWasi = runWasi(positive, positiveBytes);
assertPositive(positive, "native", positiveNative, positiveRequestValue);
const positiveParsed = assertPositive(positive, "wasi", positiveWasi, positiveRequestValue);
const positiveComparison = compare(positive, "full", positiveNative, positiveWasi);
check(positiveComparison.record.moduleGraphEqual, "positive: moduleGraph bytes differ");
check(positiveComparison.record.tableEqual, "positive: table bytes differ");
check(positiveComparison.record.wholeResponseEqual, "positive: whole responses differ");
positiveComparison.record.answered = {
  native: assertAnswered("positive/native-recorded", JSON.parse(positiveNative.bytes.toString("utf8")).table, positiveRequestValue.demands[0]),
  wasi: assertAnswered("positive/wasi-recorded", positiveParsed.table, positiveRequestValue.demands[0]),
};

// 2. No graph, over the positive case's own frozen tree (so no path differs):
// the original response shape, and the same table bytes as with the graph.
{
  const tree = { ...positive, name: "no-graph", caseRoot: join(out, "runs", "no-graph") };
  mkdirSync(tree.caseRoot, { recursive: true });
  const request = { ...positiveRequestValue };
  delete request.moduleGraph;
  const bytes = encodeRequest(tree, request);
  const native = runNative(tree, bytes);
  const wasi = runWasi(tree, bytes);
  const { record, nativeFields, wasiFields } = compare(tree, "no-graph", native, wasi);
  check(record.wholeResponseEqual, "no-graph: whole responses differ");
  check(JSON.stringify(record.rootKeys.wasi) === JSON.stringify(["ok", "table"]), `no-graph: root keys ${record.rootKeys.wasi}`);
  record.tableEqualToGraphRun = {
    native: nativeFields.get("table").equals(positiveComparison.nativeFields.get("table")),
    wasi: wasiFields.get("table").equals(positiveComparison.wasiFields.get("table")),
  };
  check(record.tableEqualToGraphRun.native && record.tableEqualToGraphRun.wasi, "no-graph: table differs from the graph run's");
  record.answered = {};
  for (const [side, response] of [["native", native], ["wasi", wasi]]) {
    record.answered[side] = assertAnswered(`no-graph/${side}`, JSON.parse(response.bytes.toString("utf8")).table, request.demands[0]);
  }
}

// 3. Zero graph demand: the inventory alone.
{
  const tree = freeze("zero-demand");
  const bytes = encodeRequest(tree, { projectId: join(tree.root, "tsconfig.json"), generation: 1, demands: [], moduleGraph: {} });
  const native = runNative(tree, bytes);
  const wasi = runWasi(tree, bytes);
  const { record } = compare(tree, "zero-demand", native, wasi);
  check(record.wholeResponseEqual, "zero-demand: whole responses differ");
  const graph = JSON.parse(wasi.bytes.toString("utf8")).moduleGraph;
  check(graph.modules.length > 0 && graph.imports === undefined && graph.unknownImportPaths === undefined,
    `zero-demand: keys ${Object.keys(graph)}`);
}

// 4. Omission: one demanded source exists on disk and is missing from memfs
// alone, under the identical request.
{
  const tree = freeze("omission");
  const omitted = join(tree.root, "src/param-bound.js");
  const request = positiveRequest(tree);
  const bytes = encodeRequest(tree, request);
  const native = runNative(tree, bytes);
  const nativeParsed = assertPositive(tree, "native-intact", native, request);
  check(nativeParsed.moduleGraph.imports.some(fact => fact.specifier.path === omitted),
    "omission: the intact native result lacks the row it is meant to lose");
  const wasi = runWasi(tree, bytes, new Set([omitted]));
  assertPositive(tree, "wasi-omitted", wasi, request, new Set([omitted]));
  const { record } = compare(tree, "omission", native, wasi);
  check(!record.wholeResponseEqual && !record.moduleGraphEqual, "omission: an omitted source left the graph unchanged");
  record.omitted = omitted;
}

// 5. Changed source: a fresh call over changed bytes changes that digest only.
{
  const tree = freeze("changed-source", root => {
    writeFileSync(join(root, "src/alias/helper.ts"), "export const helper = 2;\n");
  });
  const request = positiveRequest(tree);
  const bytes = encodeRequest(tree, request);
  const native = runNative(tree, bytes);
  const wasi = runWasi(tree, bytes);
  const { record } = compare(tree, "changed-source", native, wasi);
  check(record.wholeResponseEqual, "changed-source: whole responses differ");
  const digests = parsed => new Map(parsed.table.sources.map(source => [relative(parsed.table.projectId.slice(0, -"/tsconfig.json".length), source.path), source.sha256]));
  const before = digests(positiveParsed);
  const after = digests(JSON.parse(wasi.bytes.toString("utf8")));
  for (const [path, digest] of before) {
    check((after.get(path) === digest) === (path !== `src${sep}alias${sep}helper.ts`), `changed-source: digest of ${path}`);
  }
  check(before.size === after.size, "changed-source: source sets differ");
}

// 6. Failure: a project that cannot open fails with no table and no graph.
{
  const tree = freeze("failure");
  const bytes = encodeRequest(tree, {
    projectId: join(tree.root, "missing", "tsconfig.json"),
    generation: 1,
    demands: [],
    moduleGraph: { imports: true },
  });
  const native = runNative(tree, bytes);
  const wasi = runWasi(tree, bytes);
  const { record } = compare(tree, "failure", native, wasi);
  for (const [side, response] of [["native", native], ["wasi", wasi]]) {
    const keys = [...rootFields(response.bytes).keys()];
    check(response.status === 1 && JSON.stringify(keys) === JSON.stringify(["ok", "error"]), `failure: ${side} status ${response.status} keys ${keys}`);
  }
  record.errors = {
    native: JSON.parse(native.bytes.toString("utf8")).error,
    wasi: JSON.parse(wasi.bytes.toString("utf8")).error,
  };
}

// 7–8. Filesystem-fidelity controls: recorded either way, never normalized.
function recorded(name, mutate, file) {
  const tree = freeze(name, mutate);
  const bytes = encodeRequest(tree, {
    projectId: join(tree.root, "tsconfig.json"),
    generation: 1,
    demands: [],
    moduleGraph: { imports: true, packages: true, importPaths: [join(tree.root, file)] },
  });
  const native = runNative(tree, bytes);
  const wasi = runWasi(tree, bytes);
  const { record } = compare(tree, name, native, wasi);
  const rows = response => {
    const parsed = JSON.parse(response.bytes.toString("utf8"));
    return parsed.ok ? parsed.moduleGraph.imports ?? [] : parsed.error;
  };
  record.recordedOnly = true;
  record.rows = { native: rows(native), wasi: rows(wasi) };
}
recorded("symlink", root => {
  mkdirSync(join(root, "vendor", "linked-real"), { recursive: true });
  writeFileSync(join(root, "vendor", "linked-real", "package.json"), '{ "name": "linked", "version": "4.5.6", "types": "index.d.ts" }\n');
  writeFileSync(join(root, "vendor", "linked-real", "index.d.ts"), "export declare const linked: number;\n");
  symlinkSync(join("..", "vendor", "linked-real"), join(root, "node_modules", "linked"));
  writeFileSync(join(root, "src", "linked-use.ts"), 'import { linked } from "linked";\nexport const viaLink = linked;\n');
}, "src/linked-use.ts");
recorded("case", root => {
  writeFileSync(join(root, "src", "case-use.ts"), 'import { helper } from "./Alias/helper";\nexport const viaCase = helper;\n');
}, "src/case-use.ts");

const summary = {
  question: "Does the canonical producer's module graph answer identically as a native process and as a WASI reactor over the same bytes?",
  repositoryRoot,
  out,
  images,
  fixtureManifest: manifest(snapshot(fixture)),
  records,
  failures,
};
writeFileSync(join(out, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
console.log(JSON.stringify({ out, images, records: records.map(({ rows, ...rest }) => rest), failures }, null, 2));
if (failures.length !== 0) process.exit(1);
