// The WASM parity gate (P5 S2): the owned check, packed and installed in
// isolation, against the native ordinary check with --no-package-contracts,
// over the same frozen project bytes at the same absolute paths.
//
// It builds nothing. `make wasm-parity` builds the package, the native
// checker and the bridge test's recorded update frames first; a missing
// artifact, toolchain or input fails here rather than skipping. Nothing is
// cached. The legacy cases carry the exact expectations transplanted from the
// S1b consumer witness (scripts/wasm-parity/expectations.json); the only
// rendering is the literal case root (`<root>`, and `<root-relative>` in tsc
// output). Any other difference fails and needs a disposition, never an
// update. See scripts/wasm-parity/README.md.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  appendFileSync, chmodSync, copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, renameSync,
  readdirSync, readlinkSync, realpathSync, rmSync, symlinkSync, writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import * as pathSemantics from "./wasm-parity/path-semantics.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const gateFiles = join(repositoryRoot, "scripts/wasm-parity");
const packageRoot = join(repositoryRoot, "packages/wasm");
const { values: options } = parseArgs({
  options: {
    scratch: { type: "string" },
    "update-frames": { type: "string" },
    keep: { type: "boolean" },
  },
});
const failures = [];
const check = (condition, message) => {
  if (!condition) failures.push(message);
  return condition;
};
const fail = message => {
  console.error(`wasm-parity: ${message}`);
  process.exit(1);
};
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// --- inputs: every one present, or the gate fails --------------------------
if (!options.scratch || !options["update-frames"]) fail("--scratch and --update-frames are required");
const nativeChecker = process.env.SOLID_CHECKER_BIN;
const nativeTypefacts = process.env.SOLID_TYPEFACTS_BIN;
if (!nativeChecker || !existsSync(nativeChecker)) fail(`SOLID_CHECKER_BIN ${nativeChecker} is not a built native checker`);
if (!nativeTypefacts || !existsSync(nativeTypefacts)) fail(`SOLID_TYPEFACTS_BIN ${nativeTypefacts} is not a built producer`);
const which = name => {
  const result = spawnSync("sh", ["-c", `command -v ${name}`], { encoding: "utf8" });
  return result.status === 0 ? realpathSync(result.stdout.trim()) : undefined;
};
const nodePath = process.env.PROBE_NODE ? realpathSync(process.env.PROBE_NODE) : which("node");
const bunPath = process.versions.bun ? realpathSync(process.execPath) : which("bun");
if (!nodePath) fail("no node executable");
if (!bunPath) fail("no bun executable");
const scratch = realpathSync(options.scratch);
const updateFrames = realpathSync(options["update-frames"]);
if (!existsSync(join(updateFrames, "frames.json"))) fail(`${updateFrames}/frames.json is missing: the bridge test recorded no update frames`);
const recording = JSON.parse(readFileSync(join(updateFrames, "frames.json"), "utf8"));
const oracleInstall = join(repositoryRoot, "rust/target/tsc-oracle/v2/node_modules");
const tscPath = join(repositoryRoot, "packages/cli/node_modules/typescript/bin/tsc");
const expectations = JSON.parse(readFileSync(join(gateFiles, "expectations.json"), "utf8"));
const OLD_FILES = [
  "node.cjs", "browser-entry.js", "index.d.ts", "solid-checker-wasm.wasi.cjs",
  "solid-checker-wasm.wasi-browser.js", "solid-checker-wasm.wasm32-wasi.wasm", "wasi-worker.mjs",
  "wasi-worker-browser.mjs", "scripts",
];
for (const required of [
  oracleInstall, tscPath, join(packageRoot, "owned/manifest.json"), join(packageRoot, "node_modules"),
  ...OLD_FILES.map(file => join(packageRoot, file)),
]) {
  if (!existsSync(required)) fail(`${required} is missing; build it first (make wasm-parity)`);
}
const builtManifest = JSON.parse(readFileSync(join(packageRoot, "owned/manifest.json"), "utf8"));
for (const entry of Object.values(builtManifest.artifacts)) {
  const path = join(packageRoot, "owned", entry.file);
  if (!existsSync(path) || sha256(readFileSync(path)) !== entry.sha256) fail(`owned/${entry.file} does not match the built manifest`);
}

// Nothing above an isolated tree may supply an input: no ancestor holds a
// package install, a project or package manifest, or checker configuration.
const ANCESTOR_INPUTS = ["node_modules", "package.json", "tsconfig.json", "jsconfig.json", ".solid-checker"];
function cleanAncestors(path) {
  for (let directory = dirname(path); ; directory = dirname(directory)) {
    for (const name of ANCESTOR_INPUTS) {
      if (existsSync(join(directory, name))) fail(`${join(directory, name)} lies above the isolated tree ${path}`);
    }
    if (directory === dirname(directory)) return;
  }
}

const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const evidence = join(repositoryRoot, "rust/target/wasm-parity", stamp);
const consumer = join(scratch, "consumer");
const cases = join(scratch, "cases");
mkdirSync(evidence, { recursive: true });
mkdirSync(join(consumer, "node_modules/solid-checker-wasm"), { recursive: true });
mkdirSync(cases, { recursive: true });
for (const tree of [consumer, cases, recording.root]) cleanAncestors(tree);

// --- the packed package, installed in isolation ------------------------------
const run = (command, args, settings = {}) => {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 30, ...settings });
  if (result.error) fail(`${command} could not run: ${result.error.message}`);
  return result;
};
const pack = run(bunPath, ["pm", "pack", "--destination", join(scratch, "pack"), "--quiet"], { cwd: packageRoot });
if (pack.status !== 0) fail(`bun pm pack failed: ${pack.stderr}`);
const tarballs = readdirSync(join(scratch, "pack")).filter(name => name.endsWith(".tgz"));
if (tarballs.length !== 1) fail(`bun pm pack wrote ${tarballs.length} tarballs`);
const tarball = join(scratch, "pack", tarballs[0]);
const installed = join(consumer, "node_modules/solid-checker-wasm");
if (run("tar", ["-xzf", tarball, "-C", installed, "--strip-components", "1"]).status !== 0) fail("the tarball did not extract");
const packedManifest = JSON.parse(readFileSync(join(installed, "package.json"), "utf8"));
const packed = {
  tarball: { file: tarballs[0], sha256: sha256(readFileSync(tarball)) },
  main: packedManifest.main,
  browser: packedManifest.browser,
  types: packedManifest.types,
  exports: packedManifest.exports,
  dependencies: packedManifest.dependencies,
  oldFiles: Object.fromEntries(OLD_FILES.map(file => [file, existsSync(join(installed, file))])),
  owned: JSON.parse(readFileSync(join(installed, "owned/manifest.json"), "utf8")),
};
check(packed.main === "node.cjs" && packed.browser === "browser-entry.js" && packed.types === "index.d.ts" && packed.exports === undefined,
  `packed entry fields moved: ${JSON.stringify({ main: packed.main, browser: packed.browser, types: packed.types, exports: packed.exports })}`);
check(Object.values(packed.oldFiles).every(Boolean), `packed package lost a pre-S2 file: ${JSON.stringify(packed.oldFiles)}`);
check(same(packed.owned, builtManifest), "the packed owned manifest is not the built one");

// The declared production closure, copied from the frozen local install and
// checked against both lockfiles. Peers are declared dependencies too: the
// pre-S2 entry needs @napi-rs/wasm-runtime's.
const packageLock = JSON.parse(readFileSync(join(packageRoot, "package-lock.json"), "utf8"));
const bunLock = readFileSync(join(packageRoot, "bun.lock"), "utf8");
const treeDigest = root => {
  const hash = createHash("sha256");
  const walk = directory => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      const stat = lstatSync(path);
      const key = relative(root, path);
      if (stat.isSymbolicLink()) hash.update(`link ${key} ${readlinkSync(path)}\n`);
      else if (stat.isDirectory()) walk(path);
      else hash.update(`file ${key} ${sha256(readFileSync(path))}\n`);
    }
  };
  walk(root);
  return hash.digest("hex");
};
const closure = [];
const pending = Object.keys(packedManifest.dependencies ?? {});
while (pending.length) {
  const name = pending.shift();
  if (closure.some(entry => entry.name === name)) continue;
  const source = realpathSync(join(packageRoot, "node_modules", name));
  const manifest = JSON.parse(readFileSync(join(source, "package.json"), "utf8"));
  const locked = packageLock.packages[`node_modules/${name}`];
  const bunLocked = bunLock.includes(`"${name}": ["${name}@${manifest.version}"`);
  check(locked?.version === manifest.version && bunLocked, `${name}@${manifest.version} is not the locked version (${locked?.version})`);
  cpSync(source, join(consumer, "node_modules", name), { recursive: true });
  closure.push({ name, version: manifest.version, integrity: locked?.integrity, tree: treeDigest(source) });
  const optional = manifest.peerDependenciesMeta ?? {};
  pending.push(...Object.keys(manifest.dependencies ?? {}),
    ...Object.keys(manifest.peerDependencies ?? {}).filter(peer => !optional[peer]?.optional));
}

// The gate's own files beside the install, so their imports resolve there.
for (const file of ["driver.mjs", "fault-worker.mjs", "cbor.mjs", "url-hooks.mjs", "old-entry.mjs", "path-semantics.mjs"]) {
  copyFileSync(join(gateFiles, file), join(consumer, file));
}
// Corrupted copies of the package: one artifact byte flipped, and the same
// with its manifest digest rewritten to match.
const corruptPackages = {};
for (const [name, rewriteManifest] of [["init-corrupt-digest", false], ["init-corrupt-module", true]]) {
  const directory = join(consumer, name, "node_modules/solid-checker-wasm");
  cpSync(installed, directory, { recursive: true });
  const artifact = join(directory, "owned/solid-checker.wasm");
  chmodSync(artifact, 0o644);
  const bytes = readFileSync(artifact);
  bytes[8] ^= 0xff;
  writeFileSync(artifact, bytes);
  if (rewriteManifest) {
    const manifest = JSON.parse(readFileSync(join(directory, "owned/manifest.json"), "utf8"));
    manifest.artifacts.checker.sha256 = sha256(bytes);
    writeFileSync(join(directory, "owned/manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  }
  corruptPackages[name] = directory;
}

// --- frozen case trees ----------------------------------------------------------
function snapshotTree(root) {
  const entries = new Map();
  const walk = directory => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      const stat = lstatSync(path);
      if (stat.isSymbolicLink()) entries.set(path, `link:${readlinkSync(path)}`);
      else if (stat.isDirectory()) {
        entries.set(path, "dir");
        walk(path);
      } else entries.set(path, sha256(readFileSync(path)));
    }
  };
  walk(root);
  return Object.fromEntries(entries);
}
function freeze(name, source, mutate = () => {}) {
  const caseRoot = join(cases, name);
  const root = join(caseRoot, "project");
  mkdirSync(caseRoot, { recursive: true });
  if (typeof source === "string") cpSync(source, root, { recursive: true });
  else source(root);
  mutate(root);
  const manifest = snapshotTree(root);
  writeFileSync(join(caseRoot, "mount-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  return { name, caseRoot, root, manifest };
}
const fixture = name => join(repositoryRoot, "fixtures/reactive-ir", name);
const oracleCases = JSON.parse(readFileSync(join(repositoryRoot, "fixtures/tsc-oracle/rule-cases.json"), "utf8")).cases;
const oracleCode = rule => {
  const found = oracleCases.find(item => item.rule === rule && item.dialect === "v2" && item.expect === "silent" && item.checker === "reports");
  if (!found) fail(`no oracle keystone for ${rule}`);
  return found.code.endsWith("\n") ? found.code : `${found.code}\n`;
};
const typedProject = files => root => {
  mkdirSync(root, { recursive: true });
  cpSync(oracleInstall, join(root, "node_modules"), { recursive: true, dereference: true });
  writeFileSync(join(root, "tsconfig.json"), `${JSON.stringify({
    compilerOptions: {
      target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
      jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], skipLibCheck: true,
      allowJs: false, baseUrl: ".", strict: true, noEmit: true,
    },
    files: Object.keys(files),
  }, null, 2)}\n`);
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
};
const w1Files = {
  "Gate.tsx": oracleCode("components-return-once"),
  "Row.tsx": oracleCode("no-destructure"),
  "Hello.tsx": "export function Hello() {\n  return <div>hello</div>;\n}\n",
};
const respellImporter = (root, spelling) => {
  const path = join(root, ".solid-checker/accepted-contracts.json");
  const catalog = JSON.parse(readFileSync(path, "utf8"));
  for (const entry of catalog.contracts) entry.import.importer = spelling.replaceAll("{project}", root);
  writeFileSync(path, `${JSON.stringify(catalog, null, 2)}\n`);
};
const dangling = relativePath => root => {
  rmSync(join(root, relativePath), { recursive: true });
  symlinkSync("missing-target", join(root, relativePath));
};
const usesGate = 'import { Gate } from "./Gate";\nexport function Use() {\n  return <Gate />;\n}\n';
const TREES = {
  "w1-real-typings-violation": [typedProject(w1Files)],
  "w2-real-typings-clean": [typedProject({ "Hello.tsx": w1Files["Hello.tsx"] })],
  "w2-uncertifiable": [fixture("package-contract-absent")],
  "w3-dependency-source-program": [fixture("dependency-source-program")],
  "w3-visible-js": [fixture("dependency-source-integrity-r2-visible-js")],
  "p1-unknown-callback": [fixture("package-unknown-callback-consumer")],
  "p1-paths-shadow": [fixture("package-contract-paths-shadow")],
  "p1-install-shapes": [fixture("package-contract-install-shapes")],
  ...Object.fromEntries([
    ["absolute", "{project}/App.ts"], ["dotdot", "{project}/node_modules/../App.ts"], ["symlink", "alias/App.ts"],
  ].map(([variant, spelling]) => [`p1-key-${variant}`, [fixture("package-unknown-callback-consumer"), root => {
    symlinkSync(".", join(root, "alias"));
    respellImporter(root, spelling);
  }]])),
  "refuse-dangling-catalog-root": [fixture("package-unknown-callback-consumer"), dangling(".solid-checker")],
  "refuse-dangling-plain-catalog": [fixture("package-unknown-callback-consumer"), dangling(".solid-checker/accepted-contracts.json")],
  "refuse-dangling-installation": [fixture("package-contract-install-shapes"), dangling("node_modules/ambient-package")],
  "install-absent": [fixture("package-contract-install-shapes"), root => rmSync(join(root, "node_modules/ambient-package"), { recursive: true })],
  "w4-omission": [fixture("dependency-source-program"), root => rmSync(join(root, "node_modules/dep-hooks/src/read.js"))],
  "w4-project-intact": [typedProject({ ...w1Files, "Use.tsx": usesGate })],
  "w4-project-omitted": [typedProject({ ...w1Files, "Use.tsx": usesGate }), root => {
    rmSync(join(root, "Gate.tsx"));
    const config = JSON.parse(readFileSync(join(root, "tsconfig.json"), "utf8"));
    config.files = config.files.filter(name => name !== "Gate.tsx");
    writeFileSync(join(root, "tsconfig.json"), `${JSON.stringify(config, null, 2)}\n`);
  }],
  "w6-changed-fresh": [typedProject({
    ...w1Files,
    "Gate.tsx": w1Files["Gate.tsx"]
      .replace('import { createSignal } from "solid-js";', 'import { Show, createSignal } from "solid-js";')
      .replace("  if (ready()) return <div>on</div>;\n  return <span>off</span>;\n",
        "  return <Show when={ready()} fallback={<span>off</span>}><div>on</div></Show>;\n"),
  })],
  "w7-missing-project": [typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }), root => rmSync(join(root, "tsconfig.json"))],
  "w7-open-failure": [typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }),
    root => writeFileSync(join(root, "tsconfig.json"), '{ "compilerOptions": { "strict": true }, "files": 7 }\n')],
  "w5-faults": [fixture("dependency-source-program")],
  "w5-decoy": [typedProject({ "Hello.tsx": w1Files["Hello.tsx"] })],
  "spellings": [typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }), root => symlinkSync(".", join(root, "alias"))],
  "sc9013": [join(repositoryRoot, "rust/crates/solid-facts-backend/tests/fixtures/unsupported-runtime-v1")],
  // S3a: the `../` dependency-source fixture, and the same tree with the
  // imported module missing.
  "dotdot-fixture": [fixture("dependency-source-dotdot")],
  "dotdot-missing": [fixture("dependency-source-dotdot"), root => rmSync(join(root, "node_modules/dep-dotdot/src/lib/read.js"))],
};
const trees = Object.fromEntries(Object.entries(TREES).map(([name, [source, mutate]]) => [name, freeze(name, source, mutate)]));

// --- the native reference -------------------------------------------------------
function caseRequest(tree, projectId = join(tree.root, "tsconfig.json")) {
  const metadataPath = join(tree.root, ".solid-checker", "runtime.json");
  const metadata = existsSync(metadataPath) && lstatSync(metadataPath).isFile() ? JSON.parse(readFileSync(metadataPath, "utf8")) : {};
  const argv = ["--format", "json", "--project", projectId, "--no-package-contracts"];
  const runtime = {};
  for (const [key, flag] of [["target", "--runtime-target"], ["build", "--runtime-build"],
    ["rendering", "--rendering"], ["programBoundary", "--program-boundary"]]) {
    if (metadata[key] !== undefined) {
      argv.push(flag, metadata[key]);
      runtime[key] = metadata[key];
    }
  }
  for (const condition of metadata.conditions ?? []) argv.push("--runtime-condition", condition);
  for (const transform of metadata.frameworkTransforms ?? []) argv.push("--framework-transform", transform);
  if (metadata.conditions) runtime.conditions = metadata.conditions;
  if (metadata.frameworkTransforms) runtime.frameworkTransforms = metadata.frameworkTransforms;
  const request = { projectId, runtime, dependencySources: metadata.dependencySources ?? [] };
  for (const specifier of request.dependencySources) argv.push("--dependency-source", specifier);
  if (metadata.dependencySource === false) {
    argv.push("--no-dependency-source");
    request.noDependencySource = true;
  }
  if (metadata.realm !== undefined) {
    argv.push("--runtime-realm", metadata.realm);
    request.windowRealm = metadata.realm === "window";
  }
  return { argv, request };
}
function runNative(tree, argv, label = tree.name) {
  const result = spawnSync(nativeChecker, argv, {
    cwd: repositoryRoot,
    env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: nativeTypefacts },
    maxBuffer: 1 << 30,
  });
  const directory = join(evidence, "native", label);
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "stdout"), result.stdout);
  writeFileSync(join(directory, "stderr"), result.stderr);
  check(same(snapshotTree(tree.root), tree.manifest), `${label}: the disk tree moved during the native run`);
  const stderr = result.stderr.toString("utf8");
  return {
    status: result.status,
    stdout: result.stdout.toString("utf8"),
    snapshot: result.status === 0 ? JSON.parse(result.stdout.toString("utf8")) : undefined,
    notes: stderr === "" ? [] : stderr.replace(/\n$/, "").split("\n"),
  };
}
const tsc = tree => {
  const result = spawnSync(nodePath, [tscPath, "--noEmit", "-p", join(tree.root, "tsconfig.json")], { cwd: repositoryRoot, encoding: "utf8" });
  return { status: result.status, output: (result.stdout + result.stderr).trim() };
};
const render = (value, tree) => {
  const text = JSON.stringify(value).replaceAll(relative(repositoryRoot, tree.root), "<root-relative>").replaceAll(tree.root, "<root>");
  return JSON.parse(text);
};

const STANDARD = [...Object.keys(expectations.answering), ...Object.keys(expectations.refusals)];
const native = {};
const requests = {};
for (const name of STANDARD) {
  const tree = trees[name];
  requests[name] = caseRequest(tree);
  native[name] = runNative(tree, requests[name].argv);
}
const tscResults = {};
for (const name of ["w1-real-typings-violation", "w2-real-typings-clean", "w4-project-intact", "w4-project-omitted", "w6-changed-fresh"]) {
  tscResults[name] = tsc(trees[name]);
}
tscResults["w4-omission"] = { intact: tsc(trees["w3-dependency-source-program"]), omitted: tsc(trees["w4-omission"]) };
const w5Request = caseRequest(trees["w5-faults"]);
const w5Reference = runNative(trees["w5-faults"], w5Request.argv);
const spellingRoot = trees.spellings.root;
const SPELLINGS = [
  ["spelling-alias", join(spellingRoot, "alias/tsconfig.json")],
  ["spelling-dotdot", join(spellingRoot, "node_modules/../tsconfig.json")],
];
for (const [name, projectId] of SPELLINGS) native[name] = runNative(trees.spellings, caseRequest(trees.spellings, projectId).argv, name);
native.sc9013 = runNative(trees.sc9013, caseRequest(trees.sc9013).argv, "sc9013");

// --- S3c: project spellings through host links outside the directory root -----------
// Each tree's root is `<case>/project`; the links, the other project and the
// permission shapes sit beside it, outside the root. Every spelling runs
// natively as itself; the two permission trees are denied only between their
// native runs and the end of both engines' checks.
const g3Files = { "Hello.tsx": w1Files["Hello.tsx"] };
const besideRoot = (root, name) => join(root, "..", name);
const g3 = freeze("g3", typedProject(g3Files), root => {
  symlinkSync("project", besideRoot(root, "alias"));
  symlinkSync("hop2", besideRoot(root, "hop1"));
  symlinkSync("project", besideRoot(root, "hop2"));
  symlinkSync("missing-target", besideRoot(root, "dangling"));
  writeFileSync(besideRoot(root, "file"), "x\n");
  symlinkSync("loopB", besideRoot(root, "loopA"));
  symlinkSync("loopA", besideRoot(root, "loopB"));
  typedProject(g3Files)(besideRoot(root, "other"));
  symlinkSync("other", besideRoot(root, "elsewhere"));
  symlinkSync("project/TSCONFIG.JSON", besideRoot(root, "variant"));
});
const g3Locked = freeze("g3-locked", typedProject(g3Files), root => {
  mkdirSync(besideRoot(root, "locked"));
  symlinkSync("../project", join(besideRoot(root, "locked"), "inner"));
});
const g3Sub = freeze("g3-sub", root => {
  mkdirSync(join(root, "sub"), { recursive: true });
  typedProject(g3Files)(join(root, "sub/proj"));
}, root => symlinkSync("project/sub/proj", besideRoot(root, "subalias")));
// The OS's own link for this scratch (macOS: /var -> private/var, /tmp ->
// private/tmp); required on macOS, absent elsewhere.
const osAlias = path => (path.startsWith("/private/var/") || path.startsWith("/private/tmp/") ? path.slice("/private".length) : undefined);
if (process.platform === "darwin" && !osAlias(g3.root)) fail(`${g3.root} has no macOS /private alias`);
const g3Case = g3.caseRoot;
const G3 = {
  equal: [
    ["g3-canonical", join(g3.root, "tsconfig.json")],
    ["g3-host-link", join(g3Case, "alias/tsconfig.json")],
    ["g3-host-link-chain", join(g3Case, "hop1/tsconfig.json")],
    ["g3-host-link-dotdot", `${g3Case}/alias/../project/tsconfig.json`],
    ...(osAlias(g3.root) ? [["g3-os-alias", osAlias(join(g3.root, "tsconfig.json"))]] : []),
    // S3d: the directory's namespace is the host's, so a project outside it
    // is read as native reads it.
    ["g3-outside-root", join(g3Case, "elsewhere/tsconfig.json")],
  ],
  // [name, spelling, the package's error (exact, or a pattern), native's stderr line or "answers"]
  refused: [
    ["g3-missing", join(g3Case, "alias/absent/tsconfig.json"), "No such file or directory (os error 44)", "No such file or directory (os error 2)"],
    ["g3-dangling", join(g3Case, "dangling/tsconfig.json"), "No such file or directory (os error 44)", "No such file or directory (os error 2)"],
    ["g3-notdir", join(g3Case, "file/tsconfig.json"), / \(os error 54\)$/, "Not a directory (os error 20)"],
    ["g3-loop", join(g3Case, "loopA/tsconfig.json"), / \(os error 32\)$/, "Too many levels of symbolic links (os error 62)"],
  ],
  // Held contrasts (A4, names the host matches case-insensitively): recorded
  // beside native, never counted as equivalence.
  held: [
    ["g3-name-variant", join(g3Case, "variant")],
    ["g3-inside-case", join(g3.root, "TSCONFIG.JSON")],
  ],
};
for (const [name, projectId] of [...G3.equal, ...G3.refused, ...G3.held]) {
  native[name] = runNative(g3, caseRequest(g3, projectId).argv, name);
}
// The permission trees: the denial in place, checked real before any run.
const lockedDirectory = besideRoot(g3Locked.root, "locked");
const deniedParent = join(g3Sub.root, "sub");
chmodSync(lockedDirectory, 0o600);
chmodSync(deniedParent, 0o111);
const deniedOn = (action, code) => {
  try {
    action();
    return false;
  } catch (error) {
    return error?.code === code;
  }
};
if (!deniedOn(() => lstatSync(join(lockedDirectory, "inner")), "EACCES")) fail(`${lockedDirectory}: search is not denied on this host`);
if (!deniedOn(() => readdirSync(deniedParent), "EACCES") || !lstatSync(join(deniedParent, "proj/tsconfig.json")).isFile()) {
  fail(`${deniedParent}: not a search-only directory over an existing project on this host`);
}
const G3_PERMISSION = [
  { name: "g3-eacces", tree: g3Locked, projectId: join(lockedDirectory, "inner/tsconfig.json"),
    observe: ["L", join(lockedDirectory, "inner")], denied: { op: "lstat", path: join(lockedDirectory, "inner") } },
  { name: "g3-read-denied-parent", tree: g3Sub, projectId: join(g3Sub.caseRoot, "subalias/tsconfig.json"),
    observe: ["D", deniedParent], denied: { op: "readdir", path: deniedParent }, exists: join(deniedParent, "proj/tsconfig.json") },
];
// No manifest walk here: it would need the very permission a tree denies.
function runNativeRaw(label, argv) {
  const result = spawnSync(nativeChecker, argv, {
    cwd: repositoryRoot, env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: nativeTypefacts }, maxBuffer: 1 << 30,
  });
  const directory = join(evidence, "native", label);
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "stdout"), result.stdout);
  writeFileSync(join(directory, "stderr"), result.stderr);
  const stderr = result.stderr.toString("utf8");
  return {
    status: result.status,
    snapshot: result.status === 0 ? JSON.parse(result.stdout.toString("utf8")) : undefined,
    notes: stderr === "" ? [] : stderr.replace(/\n$/, "").split("\n"),
  };
}
for (const item of G3_PERMISSION) native[item.name] = runNativeRaw(item.name, caseRequest(item.tree, item.projectId).argv);

// --- S3d: inputs above the directory root ---------------------------------------------
// Each project is the directory the owned check is given, and what native
// discovery finds above it -- the install, the Solid runtime that selects
// the dialect, a selected dependency source -- sits outside it. Native walks
// there without bound; directory mode reads the host as native does. Each
// case's whole tree, the project and what lies beside it, is manifested
// before the runs and checked after both engines.
const hoist = root => renameSync(join(root, "node_modules"), besideRoot(root, "node_modules"));
const runtimeStub = version => `${JSON.stringify({ name: "solid-js", version })}\n`;
const A2 = {
  "a2-hoisted": freeze("a2-hoisted", typedProject(w1Files), hoist),
  "a2-unsupported-major": freeze("a2-unsupported-major", typedProject(w1Files), root => {
    hoist(root);
    writeFileSync(besideRoot(root, "node_modules/solid-js/package.json"), runtimeStub("1.9.14"));
  }),
  "a2-local-override": freeze("a2-local-override", typedProject(w1Files), root => {
    mkdirSync(besideRoot(root, "node_modules/solid-js"), { recursive: true });
    writeFileSync(besideRoot(root, "node_modules/solid-js/package.json"), runtimeStub("1.9.14"));
  }),
  "a2-absent": freeze("a2-absent", typedProject(w1Files), root => rmSync(join(root, "node_modules"), { recursive: true })),
  "a2-store-link": freeze("a2-store-link", typedProject(w1Files), root => {
    mkdirSync(besideRoot(root, ".store"));
    renameSync(join(root, "node_modules"), besideRoot(root, ".store/node_modules"));
    symlinkSync(".store/node_modules", besideRoot(root, "node_modules"));
  }),
  "a2-dependency-source": freeze("a2-dependency-source", fixture("dependency-source-program"), hoist),
  "a2-dependency-missing": freeze("a2-dependency-missing", fixture("dependency-source-program"), root => {
    hoist(root);
    rmSync(besideRoot(root, "node_modules/dep-hooks/src/read.js"));
  }),
};
const a2Before = {};
for (const [name, tree] of Object.entries(A2)) {
  a2Before[name] = snapshotTree(tree.caseRoot);
  writeFileSync(join(tree.caseRoot, "..", `${name}.case-manifest.json`), `${JSON.stringify(a2Before[name], null, 2)}\n`);
  requests[name] = caseRequest(tree);
  native[name] = runNative(tree, requests[name].argv);
}
// The same unsupported runtime above a closed mount: the mount has no
// ancestors, so it is not seen there (the legacy mount contract).
const A2_MOUNT = ["a2-hoisted", "a2-unsupported-major"];
const w2Request = caseRequest(trees["w2-real-typings-clean"]);
const DOTDOT = ["dotdot-fixture", "dotdot-missing"];
for (const name of DOTDOT) {
  requests[name] = caseRequest(trees[name]);
  native[name] = runNative(trees[name], requests[name].argv);
  tscResults[name] = tsc(trees[name]);
}
// The same sources against the published Solid 2 rc.13 types (the oracle's
// audited install) instead of the fixture's stub: TypeScript must be silent.
{
  const typed = join(scratch, "dotdot-published", "project");
  cpSync(trees["dotdot-fixture"].root, typed, { recursive: true });
  for (const stub of ["node_modules/solid-js", "node_modules/@solidjs", "solid-js.d.ts"]) rmSync(join(typed, stub), { recursive: true });
  cpSync(oracleInstall, join(typed, "node_modules"), { recursive: true, dereference: true });
  writeFileSync(join(typed, "tsconfig.json"), `${JSON.stringify({
    compilerOptions: {
      target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve", jsxImportSource: "@solidjs/web",
      lib: ["ES2022", "DOM"], types: [], skipLibCheck: true, strict: true, noEmit: true,
    },
    include: ["*.ts", "*.tsx"],
  }, null, 2)}\n`);
  tscResults["dotdot-published"] = tsc({ root: typed });
}

// --- path semantics (S3a) -----------------------------------------------------------
// The control guests run against one disk tree and its mounted copy; each
// mutating row gets a fresh guest copy and a separate host copy.
const guestBuild = join(repositoryRoot, "rust/target/wasm-parity-guests");
const pathCases = join(scratch, "paths");
mkdirSync(join(pathCases, "guests"), { recursive: true });
const guests = {};
for (const [name, built] of [["rust", join(guestBuild, "wasm32-wasip1/release/path-control.wasm")], ["go", join(guestBuild, "path-control-go.wasm")]]) {
  if (!existsSync(built)) fail(`${built} is missing; build it first (make wasm-parity)`);
  guests[name] = join(pathCases, "guests", `${name}.wasm`);
  copyFileSync(built, guests[name]);
}
const pathRoot = join(pathCases, "tree");
pathSemantics.buildTree(pathRoot);
const pathMutations = pathSemantics.mutationRows().map(row => ({
  row, guestRoot: pathSemantics.copyTree(pathRoot, "guest"), hostRoot: pathSemantics.copyTree(pathRoot, "host"),
}));
const pathStdlib = pathSemantics.stdlibRows(pathRoot);
const hostRequest = join(pathCases, "host-request.json");
writeFileSync(hostRequest, `${JSON.stringify({
  root: pathRoot,
  rows: [...pathSemantics.rows(), ...pathStdlib.rust, ...pathStdlib.go],
  mutations: pathMutations.map(({ row, hostRoot }) => ({ row, root: hostRoot })),
})}\n`);
const hostRun = run(nodePath, [join(gateFiles, "path-host.mjs"), hostRequest, join(pathCases, "host.json")]);
if (hostRun.status !== 0) fail(`the host path reference failed: ${hostRun.stderr}`);
const pathHost = JSON.parse(readFileSync(join(pathCases, "host.json"), "utf8"));
const pathExpectations = JSON.parse(readFileSync(join(gateFiles, "path-expectations.json"), "utf8"));
// The host reference outlives the scratch tree, beside the guests' raw rows.
mkdirSync(join(evidence, "paths"), { recursive: true });
for (const name of ["host-request.json", "host.json"]) copyFileSync(join(pathCases, name), join(evidence, "paths", name));

// Mounts that must refuse.
const mountCases = join(scratch, "mounts");
mkdirSync(join(mountCases, "outside-link/project"), { recursive: true });
mkdirSync(join(mountCases, "outside"), { recursive: true });
symlinkSync("../outside", join(mountCases, "outside-link/project/escape"));
mkdirSync(join(mountCases, "fifo/project"), { recursive: true });
if (run("mkfifo", [join(mountCases, "fifo/project/pipe")]).status !== 0) fail("mkfifo failed");
// A file outside the Node driver's only write grant, for the canary to try
// to create: the permission model must deny it. (Bun has no permission
// model; there it is created, inside this scratch.)
const outside = join(scratch, "canary-write.txt");
// S3d: directory mode follows the outside link as native does.
native.outsideLinkReached = runNativeRaw("directory-outside-link-reached",
  ["--format", "json", "--project", join(mountCases, "outside-link/project/escape/tsconfig.json"), "--no-package-contracts"]);

// --- the driver, under both engines -----------------------------------------------
const plan = {
  standard: STANDARD.map(name => ({ name, root: trees[name].root, request: requests[name].request })),
  cases: {
    w1: { root: trees["w1-real-typings-violation"].root, request: requests["w1-real-typings-violation"].request },
    w2: { root: trees["w2-real-typings-clean"].root, request: w2Request.request },
    missingProjectId: join(trees["w1-real-typings-violation"].root, "absent/tsconfig.json"),
    sc9013: { root: trees.sc9013.root, request: caseRequest(trees.sc9013).request },
  },
  w5: {
    mountRoot: cases,
    trees: [trees["w5-faults"].caseRoot, trees["w5-decoy"].caseRoot].map(directory => ({ directory })),
    request: w5Request.request,
    decoyProjectId: join(trees["w5-decoy"].root, "tsconfig.json"),
    checks: 2 * Object.keys(expectations.w5.results).length,
  },
  spellings: [
    ...SPELLINGS.map(([name, projectId]) => ({ name, root: spellingRoot, request: caseRequest(trees.spellings, projectId).request })),
    { name: "spelling-relative", root: spellingRoot, request: { projectId: "tsconfig.json" } },
  ],
  corruptPackages,
  mounts: { outsideLink: join(mountCases, "outside-link/project"), fifo: join(mountCases, "fifo/project") },
  outside,
  updateFrames,
  dotdot: DOTDOT.map(name => ({ name, root: trees[name].root, request: requests[name].request })),
  pathSemantics: { root: pathRoot, guests, mutations: pathMutations.map(({ row, guestRoot }) => ({ row, root: guestRoot })) },
  g3: {
    cases: [...G3.equal, ...G3.refused, ...G3.held].map(([name, projectId]) => ({ name, root: g3.root, request: caseRequest(g3, projectId).request })),
    mount: { root: g3.root, request: caseRequest(g3, join(g3Case, "alias/tsconfig.json")).request },
    permission: G3_PERMISSION.map(item => ({
      name: item.name, root: item.tree.root, request: caseRequest(item.tree, item.projectId).request,
      observe: item.observe, denied: item.denied, exists: item.exists,
    })),
  },
  a2: {
    cases: Object.entries(A2).map(([name, tree]) => ({ name, root: tree.root, request: requests[name].request })),
    mount: A2_MOUNT.map(name => ({ name: `mount:${name}`, root: A2[name].root, request: requests[name].request })),
  },
};
const DECLARED = [
  ...STANDARD, "w5-faults", "w7-bad-handshake", "lifecycle-reuse", "lifecycle-close",
  "poison-trap", "poison-exit", "poison-ledger", "poison-error", "short-capacity",
  "malformed-json", "malformed-shape", "malformed-status",
  "mount-changing-path", "mount-changing-target", "mount-mutation-during-capture", "mount-byte-copy",
  "init-corrupt-digest", "init-corrupt-module", "concurrent", "mount-outside-link", "mount-fifo",
  "mount-non-normal", "mount-duplicate", "mount-undeclared-parent", "mount-unsupported-kind",
  "option-unknown", "spelling-alias", "spelling-dotdot", "spelling-relative", "sc9013", "canary",
  "w6-update-replay", "dotdot-fixture", "dotdot-missing", "path-semantics",
  // P5 S3b: every ordinary case again in directory mode, and its link controls.
  ...[...STANDARD, "spelling-alias", "spelling-dotdot", "spelling-relative", "sc9013", "dotdot-fixture", "dotdot-missing"].map(name => `directory:${name}`),
  "directory-outside-link-unreached", "directory-outside-link-reached",
  // P5 S3c: project spellings through host links outside the directory root.
  ...[...G3.equal, ...G3.refused, ...G3.held].map(([name]) => name), "mount:g3-host-link",
  ...G3_PERMISSION.map(item => item.name),
  // P5 S3d: inputs above the directory root, and the closed mount beside them.
  ...Object.keys(A2), ...A2_MOUNT.map(name => `mount:${name}`),
];

const engines = {};
for (const engine of ["node", "bun"]) {
  const out = join(scratch, `results-${engine}`);
  mkdirSync(out, { recursive: true });
  const planPath = join(out, "plan.json");
  writeFileSync(planPath, `${JSON.stringify({ ...plan, out }, null, 2)}\n`);
  const urlLog = join(out, "urls.log");
  writeFileSync(urlLog, "");
  const args = engine === "node"
    ? [
      // Directory mode reads the host as a native check does (S3d): above
      // its directory, through links, wherever discovery leads, so reads are
      // granted everywhere. Writes are granted only to this engine's results,
      // and no process may be spawned; the canary checks both.
      "--permission", "--allow-worker", "--allow-fs-read=/",
      `--allow-fs-write=${out}`, "--import", `file://${join(consumer, "url-hooks.mjs")}`,
      join(consumer, "driver.mjs"), planPath,
    ]
    : [join(consumer, "driver.mjs"), planPath];
  const started = Date.now();
  const result = run(engine === "node" ? nodePath : bunPath, args, {
    cwd: consumer,
    env: { PATH: process.env.PATH, HOME: process.env.HOME, NODE_ENV: "production", WASM_PARITY_URL_LOG: urlLog },
  });
  writeFileSync(join(out, "driver.stdout"), result.stdout);
  writeFileSync(join(out, "driver.stderr"), result.stderr);
  if (!check(result.status === 0 && existsSync(join(out, "driver.json")), `${engine}: the driver exited ${result.status}: ${result.stderr.slice(-2000)}`)) continue;
  const oldEntry = run(engine === "node" ? nodePath : bunPath, [join(consumer, "old-entry.mjs")], {
    cwd: consumer, env: { PATH: process.env.PATH, HOME: process.env.HOME, NODE_ENV: "production" },
  });
  writeFileSync(join(out, "old-entry.stdout"), oldEntry.stdout);
  writeFileSync(join(out, "old-entry.stderr"), oldEntry.stderr);
  engines[engine] = {
    wallMs: Date.now() - started,
    driver: JSON.parse(readFileSync(join(out, "driver.json"), "utf8")),
    urls: [...new Set(readFileSync(urlLog, "utf8").split("\n").filter(Boolean))].sort(),
    oldEntry: oldEntry.status === 0 ? JSON.parse(oldEntry.stdout) : { status: oldEntry.status, stderr: oldEntry.stderr },
  };
}

// Both engines have checked and validated: lift the denials, and the trees
// they guarded are still exactly what was frozen.
chmodSync(lockedDirectory, 0o755);
chmodSync(deniedParent, 0o755);
for (const item of G3_PERMISSION) check(same(snapshotTree(item.tree.root), item.tree.manifest), `${item.name}: the tree moved during the run`);
check(lstatSync(join(lockedDirectory, "inner")).isSymbolicLink() && lstatSync(join(deniedParent, "proj/tsconfig.json")).isFile(),
  "the permission trees' paths do not exist after the denials were lifted");
for (const [name, tree] of Object.entries(A2)) check(same(snapshotTree(tree.caseRoot), a2Before[name]), `${name}: the case tree, above the root included, moved during the run`);

// --- comparison -------------------------------------------------------------------
const hostOf = (transport, faulted = false) => transport && ({
  opened: transport.opened, closed: transport.closed, exchanges: transport.exchanges, faulted, pendingResults: transport.pendingResults,
});
const findingRows = snapshot => (snapshot?.findings ?? []).map(finding => ({
  id: finding.id, rule: finding.rule, kind: finding.kind, path: finding.primaryLocation?.path,
  startByte: finding.primaryLocation?.startByte, endByte: finding.primaryLocation?.endByte,
}));
const disabledNote = "package contracts are not consumed in this check (--no-package-contracts)";
const expectRejection = (label, value, message, prefix = false) =>
  check(value?.rejected === true && (prefix ? value.message.startsWith(message) : value.message === message),
    `${label}: ${JSON.stringify(value)} is not the rejection ${JSON.stringify(message)}`);
const equalToNative = (label, result, reference) => {
  check(reference.status === 0, `${label}: native exited ${reference.status}`);
  check(result?.ok === true, `${label}: the owned check did not answer: ${JSON.stringify(result)}`);
  check(JSON.stringify(result?.snapshot) === JSON.stringify(reference.snapshot), `${label}: snapshots differ`);
  check(same(result?.notes, reference.notes), `${label}: notes ${JSON.stringify(result?.notes)} vs ${JSON.stringify(reference.notes)}`);
};

for (const name of Object.keys(expectations.answering)) {
  const tree = trees[name];
  const expected = expectations.answering[name];
  const reference = native[name];
  check(same(render(requests[name].request, tree), expected.request), `${name}: request differs from the witness's`);
  check(same(render(requests[name].argv, tree), expected.argv), `${name}: argv differs from the witness's`);
  check(reference.status === expected.native.status && reference.snapshot?.status === expected.native.snapshotStatus,
    `${name}: native ${reference.status}/${reference.snapshot?.status}`);
  check(same(render(reference.notes, tree), expected.native.notes), `${name}: native notes ${JSON.stringify(reference.notes)}`);
  check(same(render(findingRows(reference.snapshot), tree), expected.findings), `${name}: findings ${JSON.stringify(findingRows(reference.snapshot))}`);
  check(same(reference.snapshot?.metrics, expected.metrics), `${name}: metrics ${JSON.stringify(reference.snapshot?.metrics)}`);
  check(same(reference.snapshot?.premises, expected.premises), `${name}: premises ${JSON.stringify(reference.snapshot?.premises)}`);
  if (expected.tsc) check(same(render(tscResults[name], tree), expected.tsc), `${name}: tsc ${JSON.stringify(tscResults[name])}`);
  if (name.startsWith("p1-unknown") || name.startsWith("p1-key")) {
    check(reference.snapshot?.findings?.[0]?.evidence?.some(step => step.message.startsWith(disabledNote)), `${name}: no Disabled note`);
  }
  for (const [engine, { driver }] of Object.entries(engines)) {
    const result = driver.cases[name];
    equalToNative(`${engine}/${name}`, result, reference);
    check(result?.snapshot?.status === expected.wasi.snapshotStatus, `${engine}/${name}: status ${result?.snapshot?.status}`);
    check(same(render(result?.notes, tree), expected.wasi.notes), `${engine}/${name}: notes`);
    check(same(hostOf(result?.transport), expected.wasi.host), `${engine}/${name}: host ${JSON.stringify(result?.transport)}`);
    check(result?.transport?.calls?.length === expected.wasi.frames, `${engine}/${name}: ${result?.transport?.calls?.length} producer calls, witness ${expected.wasi.frames}`);
  }
}
for (const name of Object.keys(expectations.refusals)) {
  const tree = trees[name];
  const expected = expectations.refusals[name];
  const reference = native[name];
  check(reference.status === expected.native.status && reference.stdout === "", `${name}: native exited ${reference.status} with ${reference.stdout.length} stdout bytes`);
  check(same(render(reference.notes, tree), expected.native.notes), `${name}: native notes ${JSON.stringify(reference.notes)}`);
  if (expected.tsc) check(same(render(tscResults[name], tree), expected.tsc), `${name}: tsc ${JSON.stringify(tscResults[name])}`);
  for (const [engine, { driver }] of Object.entries(engines)) {
    const result = driver.cases[name];
    check(result?.ok === false && result.snapshot === undefined, `${engine}/${name}: ${JSON.stringify(result)?.slice(0, 400)}`);
    check(render(result?.error, tree) === expected.wasi.error, `${engine}/${name}: error ${JSON.stringify(result?.error)}`);
    check(same(hostOf(result?.transport), expected.wasi.host), `${engine}/${name}: host ${JSON.stringify(result?.transport)}`);
    check(same(result?.transport?.calls, expected.wasi.frames), `${engine}/${name}: calls ${JSON.stringify(result?.transport?.calls)}`);
  }
}

// Both engines ran the driver to completion; neither may be skipped.
check(same(Object.keys(engines).sort(), ["bun", "node"]), `engines that completed: ${JSON.stringify(Object.keys(engines))}`);
const held = {};
const w1Name = "w1-real-typings-violation";
for (const [engine, { driver, urls, oldEntry }] of Object.entries(engines)) {
  const at = name => driver.cases[name];
  const label = name => `${engine}/${name}`;
  const executed = Object.keys(driver.cases).sort();
  check(same(executed, [...DECLARED].sort()), `${engine}: executed cases ${JSON.stringify(executed)} are not the declared ${DECLARED.length}`);

  // W5, field for field against the witness.
  const w5 = at("w5-faults");
  const names = Object.keys(expectations.w5.results);
  check(w5?.checks?.length === 2 * names.length, `${label("w5-faults")}: ${w5?.checks?.length} checks`);
  check(w5Reference.snapshot?.status === expectations.w5.referenceStatus, `w5: native reference ${w5Reference.snapshot?.status}`);
  const checkRecords = (w5?.records ?? []).filter(item => item.type === "check");
  names.forEach((fault, position) => {
    const expected = expectations.w5.results[fault];
    const faulted = w5?.checks?.[2 * position];
    const clean = w5?.checks?.[2 * position + 1];
    const faultRecord = checkRecords.find(item => item.index === 2 * position);
    const cleanRecord = checkRecords.find(item => item.index === 2 * position + 1);
    const actual = {
      status: faulted?.ok === false ? 1 : faulted?.ok === true ? 0 : undefined,
      error: render(faulted?.error, trees["w5-faults"]),
      snapshot: faulted?.snapshot !== undefined,
      host: hostOf(faulted?.transport, faultRecord?.faulted),
      ...(faultRecord?.decoy ? { decoy: faultRecord.decoy } : {}),
      afterStatus: clean?.ok === true ? 0 : clean?.ok === false ? 1 : undefined,
      afterSnapshotEqual: JSON.stringify(clean?.snapshot) === JSON.stringify(w5Reference.snapshot),
      afterHost: hostOf(clean?.transport, cleanRecord?.faulted),
      activeProgramsAfter: cleanRecord?.activePrograms,
    };
    check(faultRecord?.name === fault, `${label("w5-faults")}: check ${2 * position} ran ${faultRecord?.name}, not ${fault}`);
    check(same(actual, expected), `${label(`w5/${fault}`)}: ${JSON.stringify(actual)} is not the witness's ${JSON.stringify(expected)}`);
  });
  const lastRecord = checkRecords.at(-1);
  check(same({ ...lastRecord?.programs, active: lastRecord?.activePrograms }, expectations.w5.programs),
    `${label("w5-faults")}: programs ${JSON.stringify(lastRecord?.programs)}`);

  const handshake = at("w7-bad-handshake");
  check(same({
    status: handshake?.result?.ok === false ? 1 : undefined,
    error: handshake?.result?.error,
    host: hostOf(handshake?.result?.transport),
  }, expectations.w7BadHandshake), `${label("w7-bad-handshake")}: ${JSON.stringify(handshake)}`);

  const replay = at("w6-update-replay");
  check(replay?.handshakeEqual === expectations.w6UpdateReplay.handshakeEqual && replay?.openStatus === 0 && replay?.closeStatus === 0,
    `${label("w6-update-replay")}: handshake ${replay?.handshakeEqual}, open ${replay?.openStatus}, close ${replay?.closeStatus}`);
  check(same(replay?.exchanges, expectations.w6UpdateReplay.exchanges), `${label("w6-update-replay")}: ${JSON.stringify(replay?.exchanges)}`);

  // Lifecycle, poison and init: the package's own controls.
  const w1Json = at(w1Name)?.json;
  const reuse = at("lifecycle-reuse");
  check(reuse?.first?.json === w1Json && reuse?.again?.json === w1Json, `${label("lifecycle-reuse")}: answers differ from the fresh check`);
  check(reuse?.refusal?.ok === false && reuse.refusal.error === "No such file or directory (os error 44)" &&
    same(reuse.refusal.transport?.calls, []), `${label("lifecycle-reuse")}: refusal ${JSON.stringify(reuse?.refusal)}`);
  const closed = at("lifecycle-close");
  for (const key of ["inFlight", "queued", "after"]) expectRejection(`${label("lifecycle-close")}.${key}`, closed?.[key], "the owned checker is closed");
  check(closed?.sameClose === true, `${label("lifecycle-close")}: close() is not idempotent`);
  for (const [mode, message, prefix] of [
    ["trap", "the owned checker is poisoned: the check trapped: RuntimeError: injected trap", false],
    ["exit", "the owned checker worker exited with code 7", false],
    ["ledger", "the owned checker is poisoned: the check left programs [1] and results [] behind", false],
    ["error", "the owned checker worker failed: injected error", false],
  ]) {
    const poisoned = at(`poison-${mode}`);
    for (const key of ["first", "queued", "after"]) expectRejection(`${label(`poison-${mode}`)}.${key}`, poisoned?.[key], message, prefix);
    check(poisoned?.exitCode !== undefined, `${label(`poison-${mode}`)}: the worker did not exit`);
  }
  // A result whose answer no reactor wrote: the active check is settled with
  // the queued and later ones, and the worker is stopped.
  for (const [mode, message, prefix] of [
    ["malformed-json", "the owned checker answered malformed output: ", true],
    ["malformed-shape", "the owned checker answered malformed output: status 0 with an unexpected answer", false],
    ["malformed-status", "the owned checker answered malformed output: status 7 with an unexpected answer", false],
  ]) {
    const malformed = at(mode);
    for (const key of ["first", "queued", "after"]) expectRejection(`${label(mode)}.${key}`, malformed?.[key], message, prefix);
    check(malformed?.first?.message === malformed?.queued?.message && malformed?.first?.message === malformed?.after?.message,
      `${label(mode)}: the active, queued and later rejections differ`);
    check(malformed?.exitCode !== undefined, `${label(mode)}: the worker did not exit`);
  }
  // Mount capture: one read per caller property; what was read is what is
  // mounted, compared with the plain mount of the same files.
  const memoryRefusal = (name, answer) => check(answer?.ok === false && answer.error === "No such file or directory (os error 44)" &&
    same(answer.transport?.calls, []), `${label(name)}: /outside is in the mount: ${JSON.stringify(answer)?.slice(0, 300)}`);
  const changingPath = at("mount-changing-path");
  check(changingPath?.plain?.ok === true && changingPath.inside?.json === changingPath.plain.json, `${label("mount-changing-path")}: ${JSON.stringify(changingPath?.inside)?.slice(0, 300)}`);
  memoryRefusal("mount-changing-path", changingPath?.outside);
  check(changingPath?.reads === 1 && changingPath?.capturedReads === 1, `${label("mount-changing-path")}: path read ${changingPath?.reads} and ${changingPath?.capturedReads} times`);
  check(same(changingPath?.captured, [["file", "/m/tsconfig.json", '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n'], ["file", "/m/App.ts", "export const answer = 42;\n"]]),
    `${label("mount-changing-path")}: captured ${JSON.stringify(changingPath?.captured)}`);
  const changingTarget = at("mount-changing-target");
  check(changingTarget?.plain?.ok === true && changingTarget.changed?.json === changingTarget.plain.json, `${label("mount-changing-target")}: ${JSON.stringify(changingTarget?.changed)?.slice(0, 300)}`);
  check(changingTarget?.reads === 1 && changingTarget?.capturedReads === 1, `${label("mount-changing-target")}: target read ${changingTarget?.reads} and ${changingTarget?.capturedReads} times`);
  check(same(changingTarget?.captured?.at(-1), ["symlink", "/m/alias", "."]), `${label("mount-changing-target")}: captured ${JSON.stringify(changingTarget?.captured)}`);
  const mutation = at("mount-mutation-during-capture");
  check(mutation?.plain?.ok === true && mutation.inside?.json === mutation.plain.json, `${label("mount-mutation-during-capture")}: ${JSON.stringify(mutation?.inside)?.slice(0, 300)}`);
  memoryRefusal("mount-mutation-during-capture", mutation?.outside);
  check(same(mutation?.captured, [
    ["file", "/m/tsconfig.json", '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n'],
    ["file", "/m/App.ts", "export const answer = 42;\n"],
    ["file", "/m/late.ts", "export {};\n"],
  ]), `${label("mount-mutation-during-capture")}: captured ${JSON.stringify(mutation?.captured)}`);
  const byteCopy = at("mount-byte-copy");
  check(byteCopy?.afterWrite?.json === byteCopy?.plain?.json && byteCopy?.plain?.ok === true, `${label("mount-byte-copy")}: the caller's later write reached the mount`);
  check(byteCopy?.captured === '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n' && /^\{+$/.test(byteCopy?.callerBuffer ?? ""),
    `${label("mount-byte-copy")}: captured ${JSON.stringify(byteCopy?.captured)}, caller ${JSON.stringify(byteCopy?.callerBuffer)}`);
  const short = at("short-capacity");
  check(short?.first?.ok === false && /host could not copy the result of handle 1$/.test(short.first.error) &&
    short.first.transport?.pendingResults?.length === 0 && same(short.first.transport?.opened, short.first.transport?.closed),
    `${label("short-capacity")}: ${JSON.stringify(short?.first)}`);
  check(short?.after?.json === w1Json, `${label("short-capacity")}: the check after the short copy differs`);
  const corruptDigest = sha256(readFileSync(join(corruptPackages["init-corrupt-digest"], "owned/solid-checker.wasm")));
  expectRejection(label("init-corrupt-digest"), at("init-corrupt-digest"),
    `owned/solid-checker.wasm is sha256:${corruptDigest}, not the manifest's sha256:${builtManifest.artifacts.checker.sha256}`);
  expectRejection(label("init-corrupt-module"), at("init-corrupt-module"), "the owned checker failed to start: CompileError", true);
  const concurrent = at("concurrent");
  check(concurrent?.a?.json === w1Json && concurrent?.b?.json === at("w2-real-typings-clean")?.json, `${label("concurrent")}: answers differ`);

  for (const [name, message] of [
    ["mount-outside-link", `mount link ${plan.mounts.outsideLink}/escape -> ../outside leaves the mount root ${plan.mounts.outsideLink}`],
    ["mount-fifo", `mount entry ${plan.mounts.fifo}/pipe is not a regular file, directory or link`],
    ["mount-non-normal", 'mount entry "/m/a/../b" is not a normalized absolute POSIX path'],
    ["mount-duplicate", "mount entry /m/a is declared twice"],
    ["mount-undeclared-parent", "mount entry /m/x/y has no declared parent directory /m/x"],
    ["mount-unsupported-kind", 'mount entry /m/p has unsupported kind "fifo"'],
    ["option-unknown", 'createOwnedChecker has no option "adapters"'],
  ]) expectRejection(label(name), at(name), message);

  for (const [name] of SPELLINGS) equalToNative(label(name), at(name), native[name]);
  const relativeSpelling = at("spelling-relative");
  check(relativeSpelling?.ok === false && relativeSpelling.error === "project tsconfig.json is not an absolute path" &&
    same(relativeSpelling.transport?.calls, []), `${label("spelling-relative")}: ${JSON.stringify(relativeSpelling)}`);
  equalToNative(label("sc9013"), at("sc9013"), native.sc9013);
  check(same(at("sc9013")?.transport?.calls, []), `${label("sc9013")}: the producer was reached`);
  check(at("sc9013")?.snapshot?.findings?.map(finding => finding.id).join() === "SC9013", `${label("sc9013")}: not the SC9013 refusal`);

  // S3a fixture: native and owned answer the same exact violation at the
  // positive call, nothing at the negative, and TypeScript is silent against
  // the stub and the published types.
  {
    const tree = trees["dotdot-fixture"];
    const source = readFileSync(join(tree.root, "App.tsx"), "utf8");
    const start = source.indexOf("useLabel(name)");
    const expectedRows = [{ id: "SC1001", rule: "strict-read-untracked", kind: "violation", path: join(tree.root, "App.tsx"), startByte: start, endByte: start + "useLabel(name)".length }];
    equalToNative(label("dotdot-fixture"), at("dotdot-fixture"), native["dotdot-fixture"]);
    check(native["dotdot-fixture"].snapshot?.status === "violation" && same(findingRows(native["dotdot-fixture"].snapshot), expectedRows),
      `dotdot-fixture: native ${JSON.stringify(findingRows(native["dotdot-fixture"].snapshot))}`);
    check(at("dotdot-fixture")?.transport?.exchanges > 0, `${label("dotdot-fixture")}: the producer was not reached`);
    const missing = trees["dotdot-missing"];
    const message = `--dependency-source "dep-dotdot": ${missing.root}/node_modules/dep-dotdot/src/hooks/index.jsx imports "../lib/read.js", which resolves to no source file`;
    check(native["dotdot-missing"].status === 2 && native["dotdot-missing"].stdout === "" && same(native["dotdot-missing"].notes, [`solid-checker-rust: ${message}`]),
      `dotdot-missing: native ${native["dotdot-missing"].status} ${JSON.stringify(native["dotdot-missing"].notes)}`);
    check(at("dotdot-missing")?.ok === false && at("dotdot-missing").error === message && at("dotdot-missing").snapshot === undefined,
      `${label("dotdot-missing")}: ${JSON.stringify(at("dotdot-missing"))?.slice(0, 400)}`);
    for (const name of ["dotdot-fixture", "dotdot-missing", "dotdot-published"]) {
      check(tscResults[name]?.status === 0 && tscResults[name].output === "", `${name}: tsc ${JSON.stringify(tscResults[name])}`);
    }
  }

  // S3a path semantics: every owned row equals host POSIX on the same tree,
  // or its pinned override; the old layer fails exactly the repaired rows.
  {
    const layers = at("path-semantics");
    const ids = Object.keys(pathHost).sort();
    check(ids.length === pathExpectations.rowCount, `${label("path-semantics")}: ${ids.length} rows, pinned ${pathExpectations.rowCount}`);
    const fails = new Set(pathExpectations.oldLayerFails);
    const notRun = new Set(pathExpectations.oldLayerNotRun);
    for (const id of ids) {
      const override = pathExpectations.overrides[id];
      const expected = override ? { errno: override.errno, detail: override.detail } : pathHost[id];
      check(same(layers?.owned?.[id], expected), `${label(`path/${id}`)}: owned ${JSON.stringify(layers?.owned?.[id])}, expected ${JSON.stringify(expected)}`);
      const old = layers?.old?.[id];
      if (notRun.has(id)) check(old === undefined, `${label(`path/${id}`)}: the old layer ran a row it cannot finish`);
      else if (fails.has(id)) check(old !== undefined && !same(old, expected), `${label(`path/${id}`)}: the old layer did not fail a repaired row: ${JSON.stringify(old)}`);
      else check(same(old, expected), `${label(`path/${id}`)}: the old layer differs on an unrepaired row: ${JSON.stringify(old)}`);
    }
    for (const id of [...fails, ...notRun, ...Object.keys(pathExpectations.overrides)]) check(id in pathHost, `path-expectations names unknown row ${id}`);
  }

  // S3c/S3d: a spelling through host links outside the root answers as
  // native, with the canonical spelling's producer traffic; every other
  // outcome is native's own failure kind before any producer program; held
  // contrasts are only recorded. A mount is closed: it keeps ENOENT.
  {
    const canonical = at("g3-canonical");
    for (const [name] of G3.equal) {
      equalToNative(label(name), at(name), native[name]);
      if (name !== "g3-outside-root") check(same(at(name)?.transport, canonical?.transport), `${label(name)}: transport ${JSON.stringify(at(name)?.transport)}`);
    }
    const refusal = (name, expected) => {
      const answer = at(name);
      const matches = typeof expected === "string" ? answer?.error === expected : expected.test(answer?.error ?? "");
      check(answer?.ok === false && matches && same(answer.transport?.calls, []), `${label(name)}: ${JSON.stringify(answer)?.slice(0, 400)}`);
    };
    for (const [name, , expected, nativeLine] of G3.refused) {
      refusal(name, expected);
      check(native[name].status !== 0 && native[name].notes.at(-1)?.endsWith(nativeLine), `${name}: native ${native[name].status} ${JSON.stringify(native[name].notes)}`);
    }
    for (const [name] of G3.held) {
      held[`${engine}:${name}`] = {
        native: { status: native[name].status, notes: native[name].notes },
        package: at(name)?.ok ? { ok: true, equalToNative: JSON.stringify(at(name).snapshot) === JSON.stringify(native[name].snapshot) && same(at(name).notes, native[name].notes) } : { ok: false, error: at(name)?.error },
      };
    }
    refusal("mount:g3-host-link", "No such file or directory (os error 44)");
    const [eacces, readDenied] = ["g3-eacces", "g3-read-denied-parent"].map(at);
    for (const [name, value] of [["g3-eacces", eacces], ["g3-read-denied-parent", readDenied]]) {
      check(value?.precondition?.denied === true && value.precondition.exists !== false, `${label(name)}: precondition ${JSON.stringify(value?.precondition)}`);
    }
    check(same(eacces?.observation, { ok: false, code: "EACCES" }), `${label("g3-eacces")}: observation ${JSON.stringify(eacces?.observation)}`);
    refusal("g3-eacces", / \(os error 2\)$/);
    check(native["g3-eacces"].status !== 0 && native["g3-eacces"].notes.at(-1)?.endsWith("Permission denied (os error 13)"), `g3-eacces: native ${JSON.stringify(native["g3-eacces"])}`);
    // Search permission is all native needs, and all the owned check now
    // uses: the denied listing is never taken.
    check(readDenied?.observation === null, `${label("g3-read-denied-parent")}: the denied listing was read: ${JSON.stringify(readDenied?.observation)}`);
    equalToNative(label("g3-read-denied-parent"), readDenied, native["g3-read-denied-parent"]);
  }

  // S3d: what native finds above the directory root, the owned check finds
  // in directory mode: the dialect's runtime (an unsupported major refuses
  // as SC9013 there too), the install, a link to it outside the root, a
  // selected dependency source, and a local install's precedence. A failure
  // native reports is the same failure. A closed mount sees none of it.
  {
    for (const [name] of Object.entries(A2)) {
      const reference = native[name];
      if (reference.status === 0) equalToNative(label(name), at(name), reference);
      else {
        const message = reference.notes.at(-1)?.replace(/^solid-checker-rust: /, "");
        check(at(name)?.ok === false && at(name).error === message, `${label(name)}: ${JSON.stringify(at(name))?.slice(0, 400)} vs native ${message}`);
      }
    }
    check(native["a2-unsupported-major"].snapshot?.findings?.map(finding => finding.id).join() === "SC9013", "a2-unsupported-major: native is not SC9013");
    check(native["a2-local-override"].snapshot?.findings?.every(finding => finding.id !== "SC9013"), "a2-local-override: native refused the project");
    check(native["a2-hoisted"].snapshot?.findings?.length > 0, "a2-hoisted: native found nothing to compare");
    for (const name of A2_MOUNT) {
      const mounted = at(`mount:${name}`);
      held[`${engine}:mount:${name}`] = {
        native: { status: native[name].status, ids: native[name].snapshot?.findings?.map(finding => finding.id) },
        package: mounted?.ok ? { ok: true, ids: mounted.snapshot?.findings?.map(finding => finding.id), equalToNative: JSON.stringify(mounted.snapshot) === JSON.stringify(native[name].snapshot) } : { ok: false, error: mounted?.error },
      };
    }
    check(at("mount:a2-unsupported-major")?.snapshot?.findings?.every(finding => finding.id !== "SC9013"), `${label("mount:a2-unsupported-major")}: a closed mount saw the runtime above it`);
  }

  // Directory mode answers exactly as the mount does for every ordinary case:
  // the same answer text and the same producer traffic, refusals included.
  for (const name of [...STANDARD, "spelling-alias", "spelling-dotdot", "spelling-relative", "sc9013", "dotdot-fixture", "dotdot-missing"]) {
    const mounted = at(name);
    const direct = at(`directory:${name}`);
    check(direct?.ok === mounted?.ok && direct?.json === mounted?.json && same(direct?.transport, mounted?.transport),
      `${label(`directory:${name}`)}: differs from the mount: ${JSON.stringify(direct)?.slice(0, 400)}`);
  }
  const unreached = at("directory-outside-link-unreached");
  check(unreached?.ok === false && unreached.error === "No such file or directory (os error 44)",
    `${label("directory-outside-link-unreached")}: ${JSON.stringify(unreached)}`);
  const reached = at("directory-outside-link-reached");
  // S3d: the link is followed on the host, as native follows it, to a
  // directory with no tsconfig.json.
  check(reached?.ok === false && reached.error === "No such file or directory (os error 44)" && same(reached.transport?.calls, []) &&
    native.outsideLinkReached.status !== 0 && native.outsideLinkReached.notes.at(-1)?.endsWith("No such file or directory (os error 2)"),
  `${label("directory-outside-link-reached")}: ${JSON.stringify(reached)} vs native ${JSON.stringify(native.outsideLinkReached)}`);

  const canary = at("canary");
  if (engine === "node") {
    check(canary?.spawn === "ERR_ACCESS_DENIED" && canary?.write === "ERR_ACCESS_DENIED", `${label("canary")}: ${JSON.stringify(canary)}`);
    const allowed = [`file://${consumer}/`];
    const strays = urls.filter(url => !url.startsWith("node:") && !allowed.some(prefix => url.startsWith(prefix)));
    check(strays.length === 0, `${engine}: modules resolved outside the isolated install: ${JSON.stringify(strays)}`);
    for (const module of ["owned-worker.mjs", "owned-worker-core.mjs", "owned-host.mjs", "owned-boundary.mjs", "owned-snapshot.mjs"]) {
      check(urls.includes(`file://${installed}/${module}`), `${engine}: ${module} was not resolved from the installed package (worker hooks absent?)`);
    }
    // The owned worker's WASI runtime is the package's own vendored copy.
    for (const module of ["vendor/wasm-util/lib/mjs/wasi/index.mjs", "vendor/wasm-util/lib/mjs/wasi/preview1.mjs", "vendor/wasm-util/lib/mjs/wasi/physical.mjs"]) {
      check(urls.includes(`file://${installed}/${module}`), `${engine}: ${module} was not resolved from the installed package`);
    }
    for (const [module, prefix] of [["@napi-rs/wasm-runtime/fs", "node_modules/@napi-rs/wasm-runtime/"]]) {
      check(urls.some(url => url.startsWith(`file://${consumer}/${prefix}`)), `${engine}: ${module} was not resolved from the isolated install`);
    }
  } else {
    // Bun has no permission model: the same worker may do both, which is why
    // the Bun row shows engine behaviour and not isolation.
    check(canary?.spawn === "allowed" && canary?.write === "allowed", `${label("canary")}: ${JSON.stringify(canary)}`);
  }
  for (const [key, url] of Object.entries(driver.resolved)) {
    check(url.startsWith(`file://${consumer}/node_modules/`), `${engine}: ${key} resolved to ${url}`);
  }
  check(oldEntry?.resolved === join(installed, "node.cjs") && same(oldEntry?.requireExports, ["checkSync", "planSync"]) &&
    oldEntry?.importCheckSync === "function" && oldEntry?.importPlanSync === "function" && oldEntry?.status === "certified",
    `${engine}: the pre-S2 entry: ${JSON.stringify(oldEntry)}`);
}

// --- evidence ----------------------------------------------------------------------
for (const engine of Object.keys(engines)) cpSync(join(scratch, `results-${engine}`), join(evidence, `results-${engine}`), { recursive: true });
const versionOf = path => run(path, ["--version"]).stdout.trim();
const summary = {
  question: "Does the packed owned check, installed in isolation, answer as the native ordinary check with --no-package-contracts over the same bytes, and do its lifecycle and negative controls hold?",
  witness: expectations.source,
  inputs: {
    nativeChecker: { path: nativeChecker, sha256: sha256(readFileSync(nativeChecker)) },
    nativeTypefacts: { path: nativeTypefacts, sha256: sha256(readFileSync(nativeTypefacts)) },
    guests: Object.fromEntries(Object.entries(guests).map(([name, path]) => [name, sha256(readFileSync(path))])),
    pathExpectationsSha256: sha256(readFileSync(join(gateFiles, "path-expectations.json"))),
    node: { path: nodePath, sha256: sha256(readFileSync(nodePath)), version: versionOf(nodePath) },
    bun: { path: bunPath, sha256: sha256(readFileSync(bunPath)), version: versionOf(bunPath) },
    updateFrames: { directory: updateFrames, root: recording.root, exchanges: recording.exchanges.length },
    expectationsSha256: sha256(readFileSync(join(gateFiles, "expectations.json"))),
  },
  packed,
  closure,
  declared: DECLARED.length,
  // S3c contrasts recorded beside native and never counted as equivalence.
  held,
  engines: Object.fromEntries(Object.entries(engines).map(([engine, value]) => [engine, {
    wallMs: value.wallMs, executed: Object.keys(value.driver.cases).length, resolved: value.driver.resolved,
    urls: value.urls.length, oldEntry: value.oldEntry, canary: value.driver.cases.canary,
  }])),
  failures,
};
writeFileSync(join(evidence, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
appendFileSync(join(evidence, "failures.txt"), failures.map(line => `${line}\n`).join(""));
console.log(JSON.stringify({ evidence, declared: DECLARED.length, engines: summary.engines, failures }, null, 2));
if (failures.length === 0 && !options.keep) rmSync(scratch, { recursive: true, force: true });
process.exit(failures.length === 0 ? 0 : 1);
