// The `tsc` oracle: answers "does TypeScript already report this?" against the
// *real published* Solid typings.
//
// AGENTS.md's absolute rule -- never report what `tsc` reports -- is only
// enforceable if that question has a reproducible answer. Fixture stubs cannot
// answer it: a stub looser than the real package manufactures a defect no real
// project can produce, and every gate stays green while a rule duplicates
// `tsc`. So this script never reads a fixture's `solid-js.d.ts`. It compiles
// the snippet against packages installed from `fixtures/tsc-oracle/packages.json`
// at the exact versions this repository audits, and refuses to run if the
// installed version is not the audited one.
//
//   bun scripts/tsc-oracle.mjs provision [--dialect v1|v2|all]
//   bun scripts/tsc-oracle.mjs check --dialect v2 --file a.tsx [--file b.tsx] [--json]
//   bun scripts/tsc-oracle.mjs check --dialect v1 --code '<snippet>' [--json]
//   bun scripts/tsc-oracle.mjs versions [--json]
//
// The compiler is TypeScript 7.1, the native `tsc`, driven through its
// `typescript/sync` API (scripts/lib/tsc-oracle-api.mjs, run under
// Node). It is a separate, pinned install (`fixtures/tsc-oracle/packages.json`,
// `typescript`), never packages/cli's `typescript` 5.9.3. The CLI's
// `--pretty false` output carries no diagnostic length, and the gate compares
// exact start..end spans, so the API is the least fragile surface that answers
// the question.
//
// Two passes run for every input: `strict` and `loose` (the same options with
// `strict: false`). Reporting them apart is what distinguishes "TypeScript
// covers this" from "TypeScript covers this only under `strict`" -- a
// distinction the absolute rule explicitly refuses to treat as an exception,
// but which a ledger entry still has to state.
//
// Diagnostics carry byte spans, because the gate compares them against a
// checker finding's span, and a line/column pair cannot express "the same
// expression" when one column counts UTF-16 units.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = join(ROOT, "fixtures/tsc-oracle/packages.json");
// Under rust/target so it shares the repository's one ignored build root; the
// oracle's installs are build products, not fixtures.
const ORACLE_ROOT = join(ROOT, "rust/target/tsc-oracle");

const API_WORKER = join(ROOT, "scripts/lib/tsc-oracle-api.mjs");
// The provisioned TypeScript install (see `typescript` in the manifest).
const TYPESCRIPT_KEY = "typescript";

const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));

const dialectSpec = (dialect) => {
  if (dialect === TYPESCRIPT_KEY) return manifest.typescript;
  const spec = manifest.dialects[dialect];
  if (!spec) {
    throw new Error(
      `unknown dialect ${dialect}; manifest declares ${Object.keys(manifest.dialects).join(", ")}`,
    );
  }
  return spec;
};

const installedVersion = (root, name) => {
  const manifestPath = join(root, "node_modules", name, "package.json");
  if (!existsSync(manifestPath)) return null;
  return JSON.parse(readFileSync(manifestPath, "utf8")).version ?? null;
};

/**
 * Every `name@version` → integrity the install's `bun.lock` resolved, or null
 * when there is no readable lockfile. Bun writes JSONC with trailing commas;
 * nothing else in the format needs more than stripping them.
 */
const lockedIntegrities = (root) => {
  const lockPath = join(root, "bun.lock");
  if (!existsSync(lockPath)) return null;
  try {
    const text = readFileSync(lockPath, "utf8").replace(/,(\s*[}\]])/g, "$1");
    const entries = Object.values(JSON.parse(text).packages ?? {});
    return entries
      .filter((entry) => Array.isArray(entry) && typeof entry[0] === "string")
      .map((entry) => ({ resolved: entry[0], integrity: entry.at(-1) }));
  } catch {
    return null;
  }
};

// Absence, version drift, and integrity drift are all hard failures, never a
// skip. A silently skipped oracle is exactly the "green gate that verifies
// nothing" trap the SOLID_TYPEFACTS_BIN canary exists to prevent.
//
// The integrity arm is what makes `expect` mean the *audited bytes* rather
// than whatever a registry serves under that version string: Bun verified each
// tarball against the integrity it locked, so a lock that names the pinned
// integrity for every copy of every audited package is an install of exactly
// the tarballs the audit record describes.
const assertProvisioned = (dialect) => {
  const spec = dialectSpec(dialect);
  const root = join(ORACLE_ROOT, dialect);
  const wrong = [];
  for (const [name, expected] of Object.entries(spec.expect)) {
    const actual = installedVersion(root, name);
    if (actual !== expected) wrong.push(`${name}: expected ${expected}, found ${actual ?? "nothing"}`);
  }
  if (spec.integrity) {
    const locked = lockedIntegrities(root);
    if (!locked) wrong.push(`bun.lock: missing or unreadable, so no installed integrity can be checked`);
    for (const [name, pinned] of Object.entries(spec.integrity)) {
      if (!locked) break;
      const copies = locked.filter(({ resolved }) => resolved.startsWith(`${name}@`));
      if (!copies.length) wrong.push(`${name}: not in bun.lock`);
      for (const { resolved, integrity } of copies) {
        // Platform packages of the native compiler are pinned by integrity
        // only, at the version of the package that depends on them.
        const version = spec.expect[name] ?? spec.integrityVersion;
        if (resolved !== `${name}@${version}` || integrity !== pinned) {
          wrong.push(`${name}: locked ${resolved} ${integrity}, expected ${name}@${version} ${pinned}`);
        }
      }
    }
  }
  if (wrong.length) {
    const detail = wrong.map((line) => `  ${line}`).join("\n");
    throw new Error(
      `tsc oracle for ${dialect} is not provisioned at the pinned versions:\n${detail}\n` +
        `run: bun scripts/tsc-oracle.mjs provision --dialect ${dialect}`,
    );
  }
  return root;
};

/**
 * Install the audited versions for `dialect`, unless they are already there.
 *
 * The short-circuit is the *existing* verification, moved in front of the
 * install instead of behind it. `assertProvisioned` is the only thing that ever
 * decided whether the tree is usable, and it does not care how the tree got
 * there -- so a tree that already passes it needs no `bun install`, and one
 * that does not gets the full install exactly as before. Anything other than a
 * clean pass falls through: a missing package, a drifted version, an
 * unreadable manifest. The check is fail-closed by construction, because
 * passing it is the whole definition of provisioned.
 *
 * `--force` reinstalls regardless, for a tree that satisfies the version check
 * but is suspected of being damaged in some way the version check cannot see.
 */
const provision = (dialect, { force = false } = {}) => {
  const spec = dialectSpec(dialect);
  const root = join(ORACLE_ROOT, dialect);
  if (!force) {
    try {
      assertProvisioned(dialect);
      const versions = Object.fromEntries(
        Object.keys(spec.expect).map((name) => [name, installedVersion(root, name)]),
      );
      console.error(`already provisioned ${dialect}: ${JSON.stringify(versions)}`);
      return versions;
    } catch {
      // Not provisioned at the audited versions -- install below, then verify.
    }
  }
  mkdirSync(root, { recursive: true });
  writeFileSync(
    join(root, "package.json"),
    `${JSON.stringify({ name: `solid-checker-tsc-oracle-${dialect}`, private: true, version: "0.0.0" }, null, 2)}\n`,
  );
  execFileSync("bun", ["install", "--ignore-scripts", "--no-progress", ...spec.install], {
    cwd: root,
    stdio: "inherit",
  });
  assertProvisioned(dialect);
  const versions = Object.fromEntries(
    Object.keys(spec.expect).map((name) => [name, installedVersion(root, name)]),
  );
  console.error(`provisioned ${dialect}: ${JSON.stringify(versions)}`);
  return versions;
};

/**
 * The one serializable compiler configuration used by both `tsc` and the
 * checker side of the gate. Keeping this JSON-shaped prevents two handwritten
 * option lists from drifting (notably `lib`, `baseUrl`, and strictness).
 */
export const oracleCompilerOptions = (dialect, strict, overrides = {}) => ({
  target: "ES2022",
  module: "ESNext",
  moduleResolution: "Bundler",
  jsx: "preserve",
  jsxImportSource: dialectSpec(dialect).jsxImportSource,
  lib: ["ES2022", "DOM"],
  // A stray `@types/*` in the install tree must not change the answer.
  types: [],
  skipLibCheck: true,
  allowJs: false,
  baseUrl: ".",
  ...overrides,
  // A case may vary emit semantics, but never the two invariants the oracle
  // compares independently.
  strict,
  noEmit: true,
});

const oraclePasses = (dialect, overrides) =>
  [
    ["strict", true],
    ["loose", false],
  ].map(([name, strict]) => ({ name, options: oracleCompilerOptions(dialect, strict, overrides) }));

/** Run the Node-side TypeScript API worker; `request` in, parsed response out. */
const callApiWorker = (request) => {
  const typescriptRoot = assertProvisioned(TYPESCRIPT_KEY);
  try {
    const stdout = execFileSync("node", [API_WORKER], {
      input: JSON.stringify({
        ...request,
        typescriptRoot: join(typescriptRoot, "node_modules", "typescript"),
      }),
      encoding: "utf8",
      maxBuffer: 1 << 28,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return JSON.parse(stdout);
  } catch (error) {
    const detail = String(error.stderr ?? "").trim() || String(error.message ?? error);
    throw new Error(`tsc oracle API worker failed (needs \`node\` on PATH): ${detail}`);
  }
};

/**
 * The smallest semantic source subject containing each byte range of `code`
 * (one worker round trip for all ranges).
 */
export const oracleSubjectSpans = (code, ranges, name = "case.tsx") =>
  ranges.length
    ? callApiWorker({ op: "subjects", name, code, ranges }).subjects.map(([startByte, endByte]) => ({
        startByte,
        endByte,
      }))
    : [];

/** Return the smallest semantic source subject containing a byte range. */
export const oracleSubjectSpan = (code, startByte, endByte, name = "case.tsx") =>
  oracleSubjectSpans(code, [[startByte, endByte]], name)[0];

/**
 * The audited install for `dialect`, verified to be at the expected versions.
 *
 * Exported so a consumer that needs a *project* rather than a compilation --
 * the oracle gate runs the checker over the same snippets -- resolves against
 * the same tree, behind the same version check, instead of reimplementing it
 * and drifting.
 *
 * @param {"v2"} dialect
 * @returns {{root: string, jsxImportSource: string}}
 */
export const oracleProject = (dialect) => ({
  root: assertProvisioned(dialect),
  jsxImportSource: dialectSpec(dialect).jsxImportSource,
});

/**
 * The pinned TypeScript install the oracle executes, verified against the
 * manifest. Gate caches key on it so a TypeScript move invalidates TypeScript's
 * cached answers.
 */
export const oracleTypescriptTree = () => assertProvisioned(TYPESCRIPT_KEY);

/**
 * Compile `inputs` against the real typings for `dialect`.
 *
 * @param {"v2"} dialect
 * @param {{name: string, code: string}[]} inputs
 * @returns diagnostics from a `strict` and a `loose` pass, plus the versions
 *          they were produced against.
 */
export const runOracle = (dialect, inputs, compilerOptionOverrides = {}) => {
  const spec = dialectSpec(dialect);
  const root = assertProvisioned(dialect);
  // An isolated temp directory: the audited install is never mutated, and the
  // snippet under test never joins a project that could supply extra types.
  const work = mkdtempSync(join(tmpdir(), `solid-tsc-oracle-${dialect}-`));
  try {
    // Node's resolver walks up for node_modules, so linking the audited tree
    // next to the sources is enough -- and keeps the install read-only.
    symlinkSync(join(root, "node_modules"), join(work, "node_modules"), "dir");
    const files = inputs.map(({ name, code }) => {
      const target = join(work, name);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, code);
      return target;
    });
    const { passes } = callApiWorker({
      op: "check",
      work,
      files,
      passes: oraclePasses(dialect, compilerOptionOverrides),
    });
    for (const diagnostics of Object.values(passes)) {
      diagnostics.sort((a, b) => a.startByte - b.startByte || a.code - b.code);
    }
    return {
      dialect,
      typescript: installedVersion(join(ORACLE_ROOT, TYPESCRIPT_KEY), "typescript"),
      versions: Object.fromEntries(
        Object.keys(spec.expect).map((name) => [name, installedVersion(root, name)]),
      ),
      inputs: inputs.map(({ name }) => name),
      passes,
    };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
};

const usage = () => {
  console.error(
    [
      "usage:",
      "  bun scripts/tsc-oracle.mjs provision [--dialect v2|all] [--force]",
      "  bun scripts/tsc-oracle.mjs check --dialect v2 (--file <path>... | --code <snippet>) [--json]",
      "  bun scripts/tsc-oracle.mjs versions [--json]",
    ].join("\n"),
  );
  process.exit(2);
};

const parseArgs = (argv) => {
  const options = { dialect: null, files: [], code: null, json: false, name: null, force: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") options.json = true;
    else if (arg === "--force") options.force = true;
    else if (arg === "--dialect") options.dialect = argv[++index];
    else if (arg === "--file") options.files.push(argv[++index]);
    else if (arg === "--code") options.code = argv[++index];
    else if (arg === "--name") options.name = argv[++index];
    else usage();
  }
  return options;
};

const main = () => {
  const [command, ...rest] = process.argv.slice(2);
  const options = parseArgs(rest);
  if (command === "provision") {
    const dialects =
      !options.dialect || options.dialect === "all"
        ? [TYPESCRIPT_KEY, ...Object.keys(manifest.dialects)]
        : [options.dialect];
    for (const dialect of dialects) provision(dialect, { force: options.force });
    return;
  }
  if (command === "versions") {
    const report = Object.fromEntries(
      Object.keys(manifest.dialects).map((dialect) => {
        const spec = dialectSpec(dialect);
        const root = join(ORACLE_ROOT, dialect);
        return [
          dialect,
          Object.fromEntries(
            Object.keys(spec.expect).map((name) => [
              name,
              { expected: spec.expect[name], installed: installedVersion(root, name) },
            ]),
          ),
        ];
      }),
    );
    console.log(
      JSON.stringify(
        {
          typescript: {
            expected: manifest.typescript.expect.typescript,
            installed: installedVersion(join(ORACLE_ROOT, TYPESCRIPT_KEY), "typescript"),
          },
          dialects: report,
        },
        null,
        2,
      ),
    );
    return;
  }
  if (command !== "check") usage();
  if (!options.dialect) usage();
  const inputs = options.code
    ? [{ name: options.name ?? "oracle.tsx", code: options.code.endsWith("\n") ? options.code : `${options.code}\n` }]
    : options.files.map((path) => ({
        name: basename(path),
        code: readFileSync(resolve(path), "utf8"),
      }));
  if (!inputs.length) usage();
  const result = runOracle(options.dialect, inputs);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }
  console.log(`dialect ${result.dialect}  typescript ${result.typescript} (native tsc)`);
  console.log(`packages ${JSON.stringify(result.versions)}`);
  for (const pass of ["strict", "loose"]) {
    const diagnostics = result.passes[pass];
    console.log(`\n--- ${pass}: ${diagnostics.length} diagnostic(s) ---`);
    for (const d of diagnostics) {
      console.log(
        `${d.file}(${d.line},${d.column}) [${d.startByte}..${d.endByte}] TS${d.code}: ${d.message}`,
      );
    }
  }
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    main();
  } catch (error) {
    console.error(String(error.message ?? error));
    process.exit(1);
  }
}
