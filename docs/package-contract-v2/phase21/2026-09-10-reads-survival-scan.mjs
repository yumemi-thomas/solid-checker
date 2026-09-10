// Survival-fraction measurement for the proposed `reads` census refusal
// (reads-veto-observation-design.md § 7): refuse an artifact case whose closure
// installs an accessor at run time, decide every other case.
//
// Unit here is the published package archive, which is a **superset** of any
// artifact-case closure inside it. So "archive clean" soundly implies every
// closure in it is clean, and the survival number below is a LOWER BOUND.
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

const repo = "/Users/thomas/Documents/Github/solid-checker";
const manifest = JSON.parse(readFileSync(join(repo, "scripts/ecosystem-benchmark/manifest.json")));
const report = JSON.parse(
  readFileSync(join(repo, "rust/target/ecosystem-investigations/2026-09-09-machine-full.json"))
);

// Exact integrity per package@version, from the manifest's own rows.
const integrityOf = new Map();
const walk = value => {
  if (!value || typeof value !== "object") return;
  if (value.package && value.version && value.integrity) {
    integrityOf.set(`${value.package}@${value.version}`, value.integrity);
  }
  for (const child of Object.values(value)) if (typeof child === "object") walk(child);
};
walk(manifest);

const cacheRoot = join(repo, "rust/target/registry-cache/v1");
const archiveFor = (name, version, integrity) => {
  const key = createHash("sha256")
    .update(JSON.stringify([manifest.registry, name, version, integrity]))
    .digest("hex");
  const path = join(cacheRoot, key.slice(0, 2), key, "package.tgz");
  return existsSync(path) ? path : null;
};

// The refusal set: every syntactic way to install an accessor at run time.
// Each member is pinned by a zero-form case in the producer's
// `TestRuntimeInstalledAccessorReadsAreInvisibleToTheProducer`; do not add a
// pattern here without one, and do not remove one that has a case.
//
// `proxy` matches the bare identifier rather than `Proxy(` because
// `Proxy.revocable(...)` is a second spelling of the same hazard, and the
// first version of this scan missed it.
const PATTERNS = {
  proxy: /\bProxy\b/,
  defineProperty: /\bdefinePropert(?:y|ies)\b/,
  defineGetter: /__define(?:Getter|Setter)__/,
  objectCreate: /\bObject\s*\.\s*create\s*\(/,
  setPrototypeOf: /\bsetPrototypeOf\b/,
  protoAssignment: /__proto__/,
  computedObjectMember: /\bObject\s*\[/,
  reflect: /\bReflect\s*\./
};
// The esbuild/rollup CJS-interop preamble installs getters on the namespace
// object with a uniform, recognizable shape. Counted separately: if it
// dominates, it is one special case rather than a reason to abandon the route.
const ESBUILD_EXPORT_HELPER =
  /__(?:def|define)Prop\s*\(\s*\w+\s*,\s*\w+\s*,\s*\{\s*get\s*:/;

const SCANNED = /\.(?:js|mjs|cjs|jsx|ts|tsx|mts|cts)$/;

const files = root => {
  const out = [];
  const recurse = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) recurse(path);
      else if (entry.isFile() && SCANNED.test(entry.name)) out.push(path);
    }
  };
  recurse(root);
  return out;
};

const rows = [];
const missing = [];
const scratch = execFileSync("mktemp", ["-d", "/private/tmp/solid-reads-survival-XXXXXX"])
  .toString()
  .trim();

for (const result of report.results) {
  const content = result.contractContent;
  if (!content || typeof content.exportsTotal !== "number") continue;
  const key = `${result.package}@${result.version}`;
  const integrity = integrityOf.get(key);
  const archive = integrity ? archiveFor(result.package, result.version, integrity) : null;
  if (!archive) {
    missing.push({ key, exports: content.exportsTotal, why: integrity ? "not cached" : "no integrity" });
    continue;
  }
  const root = join(scratch, createHash("sha256").update(key).digest("hex").slice(0, 16));
  if (!existsSync(root)) {
    execFileSync("mkdir", ["-p", root]);
    try {
      execFileSync("tar", ["-xzf", archive, "-C", root, "--strip-components=1"], { stdio: "ignore" });
    } catch {
      missing.push({ key, exports: content.exportsTotal, why: "extract failed" });
      continue;
    }
  }
  const hits = {};
  let esbuildHelper = 0;
  let scanned = 0;
  for (const file of files(root)) {
    if (statSync(file).size > 12_000_000) continue;
    const text = readFileSync(file, "utf8");
    scanned += 1;
    for (const [name, pattern] of Object.entries(PATTERNS)) {
      if (pattern.test(text)) hits[name] = (hits[name] ?? 0) + 1;
    }
    if (ESBUILD_EXPORT_HELPER.test(text)) esbuildHelper += 1;
  }
  rows.push({ key, exports: content.exportsTotal, scanned, hits, esbuildHelper });
}

const core = Object.keys(PATTERNS);
const dirty = (row, set) => set.some(name => (row.hits[name] ?? 0) > 0);

const tally = set => {
  const clean = rows.filter(row => !dirty(row, set));
  return {
    packagesClean: clean.length,
    packagesTotal: rows.length,
    exportsClean: clean.reduce((sum, row) => sum + row.exports, 0),
    exportsTotal: rows.reduce((sum, row) => sum + row.exports, 0)
  };
};

// How much of the core refusal is only the bundler preamble.
const onlyHelper = rows.filter(row => {
  const hits = core.filter(name => (row.hits[name] ?? 0) > 0);
  return hits.length === 1 && hits[0] === "defineProperty" && row.esbuildHelper > 0;
});

const byPattern = Object.fromEntries(
  Object.keys(PATTERNS).map(name => [
    name,
    {
      packages: rows.filter(row => (row.hits[name] ?? 0) > 0).length,
      exports: rows
        .filter(row => (row.hits[name] ?? 0) > 0)
        .reduce((sum, row) => sum + row.exports, 0)
    }
  ])
);

console.log(
  JSON.stringify(
    {
      basis: {
        report: "rust/target/ecosystem-investigations/2026-09-09-machine-full.json",
        unit: "published package archive (superset of any artifact-case closure inside it)",
        direction: "lower bound on survival",
        scratch
      },
      coverage: {
        rowsMeasured: rows.length,
        exportsMeasured: rows.reduce((sum, row) => sum + row.exports, 0),
        rowsMissing: missing.length,
        exportsMissing: missing.reduce((sum, row) => sum + row.exports, 0)
      },
      core: tally(core),
      byPattern,
      bundlerPreambleOnly: {
        packages: onlyHelper.length,
        exports: onlyHelper.reduce((sum, row) => sum + row.exports, 0)
      },
      missingSample: missing.slice(0, 8)
    },
    null,
    2
  )
);
