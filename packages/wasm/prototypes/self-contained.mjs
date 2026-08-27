// PROTOTYPE: prove the real TypeScript-Go producer can run as a WASI reactor
// and answer the Rust checker's exact demand plan without a child process.
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { WASI } from "@napi-rs/wasm-runtime";
import { memfs } from "@napi-rs/wasm-runtime/fs";

// @tybys/wasm-util logs expected WASI errno probes in development mode.
process.env.NODE_ENV ||= "production";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");
const typefactsRoot = repositoryRoot;
const fixtureRoot = resolve(repositoryRoot, "fixtures/reactive-ir/props-callers");
const scratch = mkdtempSync(join(tmpdir(), "solid-checker-self-contained-"));
const typeFactsReactorPath = join(scratch, "solid-typefacts.wasm");
const checkerReactorPath = resolve(
  repositoryRoot,
  "rust/target/wasm32-wasip1/release/solid_checker_wasm.wasm",
);
const sourceName = "App.tsx";
const guestRoot = "/project";
const projectId = `${guestRoot}/tsconfig.json`;
const sourcePath = `${guestRoot}/${sourceName}`;
const source = readFileSync(resolve(fixtureRoot, sourceName), "utf8");
const { fs: virtualFs } = memfs({
  [`${guestRoot}/tsconfig.json`]: readFileSync(resolve(fixtureRoot, "tsconfig.json"), "utf8"),
  [sourcePath]: source,
  [`${guestRoot}/solid-js.d.ts`]: readFileSync(resolve(fixtureRoot, "solid-js.d.ts"), "utf8"),
});

function buildTypeFactsReactor() {
  const build = spawnSync(
    "go",
    [
      "build",
      "-trimpath",
      "-buildmode=c-shared",
      "-ldflags=-s -w",
      "-o", typeFactsReactorPath,
      "./apps/solid-typefacts/prototypes/typefacts-reactor",
    ],
    {
      cwd: typefactsRoot,
      env: {
        ...process.env,
        GOOS: "wasip1",
        GOARCH: "wasm",
        GOCACHE: join(scratch, "go-build-cache"),
      },
      encoding: "utf8",
    },
  );
  if (build.status !== 0) {
    throw new Error(build.stderr || "Type Facts reactor build failed");
  }
}

buildTypeFactsReactor();

const checkerBuild = spawnSync(
  "cargo",
  [
    "+1.97",
    "build",
    "--manifest-path", "rust/Cargo.toml",
    "-p", "solid-checker-wasm",
    "--target", "wasm32-wasip1",
    "--no-default-features",
    "--features", "reactor,dialect-v1,dialect-v2",
    "--release",
  ],
  { cwd: repositoryRoot, encoding: "utf8" },
);
if (checkerBuild.status !== 0) {
  throw new Error(checkerBuild.stderr || "checker reactor build failed");
}

const sourceRequest = {
  projectId,
  dialect: "solid-v2",
  generation: 1,
  sources: [{
    path: sourcePath,
    source,
    compilerOptions: {
      moduleName: "dom",
      generate: "dom",
      hydratable: false,
      dev: false,
      effectWrapper: "",
      wrapConditionals: true,
      staticMarker: "_$",
      builtIns: [],
    },
  }],
};

async function instantiateReactor(path, options = {}) {
  const wasi = new WASI({ version: "preview1", ...options });
  const module = await WebAssembly.compile(readFileSync(path));
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
  wasi.initialize(instance);
  return instance;
}

function callReactor(instance, operation, request) {
  const encoded = new TextEncoder().encode(JSON.stringify(request));
  const inputPointer = instance.exports.allocate_input(encoded.length);
  new Uint8Array(instance.exports.memory.buffer, inputPointer, encoded.length).set(encoded);
  const status = instance.exports[operation]();
  const output = JSON.parse(new TextDecoder().decode(new Uint8Array(
    instance.exports.memory.buffer,
    instance.exports.output_pointer(),
    instance.exports.output_length(),
  )));
  if (status !== 0) throw new Error(output.error || `${operation} failed`);
  return output;
}

const checker = await instantiateReactor(checkerReactorPath);
const plan = callReactor(checker, "run_plan", sourceRequest);

const typeFacts = await instantiateReactor(typeFactsReactorPath, {
  fs: virtualFs,
  preopens: { [guestRoot]: guestRoot },
});

const encoded = new TextEncoder().encode(JSON.stringify(plan));
const inputPointer = typeFacts.exports.allocate_input(encoded.length);
new Uint8Array(typeFacts.exports.memory.buffer, inputPointer, encoded.length).set(encoded);
const status = typeFacts.exports.run_typefacts();
const outputPointer = typeFacts.exports.output_pointer();
const outputLength = typeFacts.exports.output_length();
const result = JSON.parse(new TextDecoder().decode(
  new Uint8Array(typeFacts.exports.memory.buffer, outputPointer, outputLength),
));
if (status !== 0 || result.ok !== true) throw new Error(result.error || "Type Facts reactor failed");

// Go marshals a nil slice as null and an absent optional as "". Dropping both
// is what Rust's nested fact types want: their optional fields carry serde
// defaults, and a null there would fail to deserialize.
function normalizeGoJson(value) {
  if (Array.isArray(value)) return value.map(normalizeGoJson);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).flatMap(([key, nested]) => {
    if (nested === null || nested === "") return [];
    const normalizedKey = `${key[0].toLowerCase()}${key.slice(1)}`;
    return [[normalizedKey, normalizeGoJson(nested)]];
  }));
}

// The table's own slices are the exception: Rust's TypeScriptSnapshot
// (rust/crates/solid-facts/src/project.rs) is deny_unknown_fields with no
// serde defaults, so a key the producer marshalled as null has to come back as
// an empty array rather than disappear.
const REQUIRED_TABLE_SLICES = ["sources", "entities", "symbols", "files"];

function normalizeTable(raw) {
  const table = normalizeGoJson(raw);
  for (const field of REQUIRED_TABLE_SLICES) {
    table[field] ??= [];
  }
  return table;
}

// projectId and sources come from the Type Facts reactor itself: it echoes the
// project it opened and hashes the file bytes its own TypeScript project
// resolved. ProjectFacts::join therefore compares two independently derived
// views of source identity instead of the host's view against itself.
const table = normalizeTable(result.table);
const snapshot = callReactor(checker, "run_check", { ...sourceRequest, typeFacts: table });
console.log(JSON.stringify({
  question: "Can sources alone produce Type Facts and a real checker finding through two single-threaded WASI reactors?",
  demandCount: plan.demands.length,
  sourceCount: table.sources?.length,
  entityCount: table.entities?.length,
  symbolCount: table.symbols?.length,
  fileCount: table.files?.length,
  status: snapshot.status,
  findings: snapshot.findings?.map(({ id, message }) => ({ id, message })),
}, null, 2));
