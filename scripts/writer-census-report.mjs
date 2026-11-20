#!/usr/bin/env node
// Writer-cause census report (ADR 1131's M0, measurement only): reads what scripts/writer-census.mjs kept and answers, per project, which roots may write a built-in's
// prototype under any name (the any-name writers ADR 1114 family 3 counts), why, and which rows (SC2002 uncertifiable, context `…:prototype-…`) would go if a set of
// mechanisms decided them.
//
//   node scripts/writer-census-report.mjs drops  --baseline <run dir> --mechanisms M1,M3 --out <drop dir> [--floor | --except-last]
//   node scripts/writer-census-report.mjs crosscheck --baseline <run dir> --out <what-if run dir (census on)> --mechanisms M3u
//
// A row of family 3 exists, for a kind that asks of an any-name writer, while the project has one that may reach Solid (a root that is not clear). So a mechanism frees a
// project's rows only when it decides its LAST such root, which is why a row's "first writer" (the one the message names) says nothing of what deciding it frees. Deciding a
// root is leaving it out of `prototype_writers_any`, which the checker's what-if switch does (`SOLID_CHECKER_WHATIF_WRITER_DROP`); the floor run (`drops --floor`) leaves out
// every such root, and its rows are what no mechanism on the writer side can free (named writers, and the sites that do not ask of an any-name one).
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { builtinModules } from "node:module";
import { join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";

export const isFamily3 = (reduced) => reduced[1] === "uncertifiable" && reduced[5].includes(":prototype-");

/** Every project of a run directory: `{ id, meta, rows, census }`. */
export function loadRun(directory) {
  const run = new Map();
  for (const id of readdirSync(directory).sort()) {
    const base = join(directory, id);
    if (!existsSync(join(base, "meta.json"))) continue;
    const meta = JSON.parse(readFileSync(join(base, "meta.json"), "utf8"));
    const rows = existsSync(join(base, "rows.json.gz")) ? JSON.parse(gunzipSync(readFileSync(join(base, "rows.json.gz"))).toString("utf8")) : [];
    const census = existsSync(join(base, "census.json")) ? JSON.parse(readFileSync(join(base, "census.json"), "utf8")) : null;
    run.set(id, { id, meta, rows, census });
  }
  return run;
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------
// Which files of the project are the product's: a file-name test, an estimate and never a fact (R5's tool, `coord-land/writer-decision-design.md` section 1). A writer that
// only test, story, config and script files load is the M2b question.

const TEST_DIRECTORY = /(^|\/)(__tests__|__mocks__|__fixtures__|tests?|e2e|cypress|playwright|specs?|stories|\.storybook|storybook|benchmarks?|bench|scripts?)(\/|$)/i;
const TEST_FILE = /\.(test|spec|stories|story|bench|e2e|cy)\.[cm]?[jt]sx?$/i;
const TEST_SUPPORT = /(^|\/)(testing|test-utils|test-setup|setup-tests|setupTests|vitest-setup|vitest\.setup|setup\.test|test)\.[cm]?[jt]sx?$/i;
const CONFIG_FILE = /(^|\/)[^/]*\.config\.[cm]?[jt]s$|(^|\/)\.[^/]+rc\.[cm]?[jt]s$|(^|\/)(vite|vitest|playwright|cypress|jest|webpack|rollup|tsup|panda|tailwind|postcss|eslint|prettier|babel|drizzle|knip|lingui)(\.[A-Za-z-]+)*\.[cm]?[jt]s$/i;

/** `test`, `config` or `product`, for a project file's path (made relative to the apps root first). */
export function fileClass(path) {
  if (CONFIG_FILE.test(path)) return "config";
  if (TEST_FILE.test(path) || TEST_SUPPORT.test(path) || TEST_DIRECTORY.test(path.replace(/^[^/]+\//, ""))) return "test";
  return "product";
}

const BUILTINS = new Set(builtinModules.flatMap((name) => [name, `node:${name}`]));

/** The kinds of import that load the package: a type import and one whose bindings never run (TypeScript erases both) do not. */
const LOADS = new Set(["value", "side-effect", "reexport", "dynamic", "require", "unknown"]);

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------
// Causes: the census's classes (`escape:<head>/<shape>[@receiver]`, `generated:<kind>`, `prototype-handed-on/<shape>`, `prototype-key`, `prototype-pattern`,
// `unread:<blocker>`), parsed.

export const NAME_HEADS = new Set(["globalThis", "window", "self", "top", "parent", "frames", "global", "this", "document"]);
export const ROUTE_HEADS = new Set(["source", "view", "defaultView"]);

export function parseCause(cls) {
  let match = /^escape:([^/]+)\/(.*)$/.exec(cls);
  if (match) {
    let shape = match[2];
    let receiver = "";
    const at = shape.lastIndexOf("@");
    if (at >= 0) {
      receiver = shape.slice(at + 1);
      shape = shape.slice(0, at);
    }
    const branch = shape.startsWith("branch:");
    if (branch) shape = shape.slice("branch:".length);
    return { family: "escape", head: match[1], route: ROUTE_HEADS.has(match[1]), shape, branch, receiver };
  }
  if ((match = /^generated:(.*)$/.exec(cls))) return { family: "generated", kind: match[1] };
  if ((match = /^prototype-handed-on\/(.*)$/.exec(cls))) return { family: "handed-on", shape: match[1] };
  if (cls === "prototype-key") return { family: "key" };
  if (cls === "prototype-pattern") return { family: "pattern" };
  if ((match = /^unread:(.*)$/.exec(cls))) return { family: "unread", blocker: match[1] };
  return { family: "other", cls };
}

/** The kinds of use an `alias[…]` shape lists; `null` for a shape that is no alias. */
const aliasUses = (shape) => {
  const match = /^alias\[(.*)\]$/.exec(shape);
  return match ? (match[1] === "" ? [] : match[1].split(",")) : null;
};

const NAMED_USES = ["member", "pattern", "inspected"];
const WIDE_USES = [...NAMED_USES, "computed"];
const ROUTE_USES = [...WIDE_USES, "coerced", "regexp-arg"];

/** Whether an escape shape is a use M1 follows (the read of named globals), with `uses` the kinds of use of a followed binding that are such reads too. */
function followed(shape, { simple, uses }) {
  const alias = aliasUses(shape);
  if (alias) return alias.every((use) => uses.includes(use));
  return simple.some((accepted) => (accepted.endsWith("*") ? shape.startsWith(accepted.slice(0, -1)) : shape === accepted)) && shape !== "member:valueOf";
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------
// Mechanisms (`coord-land/writer-decision-design.md` section 2). A cause-level one decides some causes (a root is decided when all of its are); a root-level one decides
// whole roots. `needs` names mechanisms whose use a cause also needs. Each is an upper bound: it says what the mechanism could decide if built to the design, not that it is.

const PROGRAM_FILE = (file) => file.startsWith("/");

export const MECHANISMS = {
  M1: { label: "follow the global object as a value: pattern, alias, member off a fallback", level: "cause", size: 2, decides: (c) => c.family === "escape" && !c.route && c.head !== "document" && followed(c.shape, { simple: ["pattern", "member:*", "inspected"], uses: NAMED_USES }) },
  M1w: { label: "M1, and a computed read followed one step", level: "cause", size: 3, includes: ["M1"], decides: (c) => c.family === "escape" && !c.route && c.head !== "document" && followed(c.shape, { simple: ["pattern", "member:*", "inspected", "computed-read"], uses: WIDE_USES }) },
  M1u: { label: "every escape of a name of the global object (the report's `+esc` bound)", level: "cause", size: 4, includes: ["M1", "M1w"], decides: (c) => c.family === "escape" && !c.route },
  M3: { label: "source/view/defaultView receivers: consumed in place, a RegExp or a literal receiver", level: "cause", size: 2, decides: (c, w) => c.family === "escape" && c.route && (["regexp", "literal"].includes(c.receiver) || followed(c.shape, { simple: ["pattern", "member:*", "inspected", "computed-read", "coerced", "new:RegExp", "argument:RegExp"], uses: ROUTE_USES })) },
  X1131: { label: "ADR 1131 alone, best case: every route in a file of the program (types discharge it), none in installed JavaScript", level: "cause", size: 2, decides: (c, w, cause) => c.family === "escape" && c.route && PROGRAM_FILE(cause.file) },
  M3u: { label: "every route (a bound: typed discharge in installed JavaScript too)", level: "cause", size: 4, includes: ["M3", "X1131"], decides: (c) => c.family === "escape" && c.route },
  M4: { label: "generated code that is not generation: Function.call.bind, an empty or constant body", level: "cause", size: 1, decides: (c) => c.family === "generated" && /^(function-member:|function-literal:(empty|const)$)/.test(c.kind) },
  M4w: { label: "M4, and literal bodies parsed with the scan (`return this` needs M1), string timers and eval literals", level: "cause", size: 2, includes: ["M4"], decides: (c, w, cause, S) => c.family === "generated" && (/^(function-member:|function-literal:(empty|const|other)$|eval-literal$|timer-string$|constructor-literal:)/.test(c.kind) || (c.kind === "function-literal:this" && ["M1", "M1w", "M1u"].some((m) => S.has(m)))) },
  M5: { label: "a prototype handed to a callee followed into it (a named writer, not any)", level: "cause", size: 2, decides: (c) => c.family === "handed-on" && /^argument:/.test(c.shape) },
  M2a: { label: "a package no value import loads (directly or through a loaded package) is no writer", level: "root", size: 2, decides: (w, p) => !p.openLoads && !w.valueLoaded },
  M2b: { label: "product realm: a package only test, config and script files load is no writer for product files (needs a ruling)", level: "root", size: 2, includes: ["M2a"], ruling: true, decides: (w, p) => !p.openProductLoads && !w.productLoaded },
  M2flag: { label: "the backend's own `loaded` flag (named by any attested import, type imports and dependency declarations included)", level: "root", size: 1, decides: (w) => !w.loaded },
  M6: { label: "unknown globals decided by the program's own census (Q2: the root may reach Solid only by an unknown global)", level: "root", size: 2, decides: (w, p) => !p.hasUnreadLoaded && Object.keys(w.problemKinds ?? {}).every((kind) => kind === "UnknownGlobal") },
  M7v: { label: "reviewed bytes for vite's client files", level: "root", size: 1, decides: (w) => w.name === "vite" },
};

/** The mechanisms of `selected`, with the ones they include. */
export function closeOver(selected) {
  const out = new Set();
  for (const id of selected) {
    out.add(id);
    for (const included of MECHANISMS[id].includes ?? []) out.add(included);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------
// One project: its effective writers (a root the checker counts as an any-name writer and that is not clear), what loads each, and what decides it.

const shortName = (writer) => writer.name ?? writer.root.split("/node_modules/").pop();

/** The project's writers with `valueLoaded`, `productLoaded` and the importers' file classes worked out, and the project's own loading facts. */
export function describe(project, appsRoot) {
  const census = project.census;
  if (!census) return { id: project.id, writers: [], openLoads: false, openProductLoads: false, opens: [], hasUnreadLoaded: false, rows: 0, family3: [], census: null };
  const relative = (file) => (file.startsWith(`${appsRoot}/`) ? file.slice(appsRoot.length + 1) : file);
  const loaders = census.loaders ?? [];
  const importersOf = (writer) => writer.importers ?? [];
  const loads = (importer) => LOADS.has(importer.kind);
  // A package is loaded by value when a project file imports it so, or a package a project file imports so reads its files in its walk.
  const valueLoadedDirectories = new Set();
  const productLoadedDirectories = new Set();
  for (const loader of loaders) {
    const importers = loader.importers.filter(loads);
    if (importers.length === 0) continue;
    const product = importers.some((importer) => fileClass(relative(importer.file)) === "product");
    for (const directory of [loader.canonical, ...loader.writers]) {
      valueLoadedDirectories.add(directory);
      if (product) productLoadedDirectories.add(directory);
    }
  }
  const writers = census.writers.map((writer) => {
    const importers = importersOf(writer);
    const valueImporters = importers.filter(loads);
    return {
      ...writer,
      name: shortName(writer),
      effective: writer.counted && !writer.clear,
      valueLoaded: valueImporters.length > 0 || valueLoadedDirectories.has(writer.canonical),
      productLoaded: valueImporters.some((importer) => fileClass(relative(importer.file)) === "product") || productLoadedDirectories.has(writer.canonical),
      importerClasses: [...new Set(valueImporters.map((importer) => fileClass(relative(importer.file))))].sort(),
      causeClasses: writer.causes.map((cause) => ({ cause, parsed: parseCause(cause.class) })),
    };
  });
  // A load no import names (a computed `import(x)`, an import no resolution answers that is no builtin of Node) may load any package: M2 keeps today's answer then.
  // `openProductLoads` is the same for the product's files alone (M2b: a test or config file's open load is no load of the product realm).
  const program = census.program ?? {};
  const computed = (program.computedLoads ?? []).map((file) => ({ file }));
  const unresolved = (program.unresolvedImports ?? []).map((entry) => ({ file: entry.slice(0, entry.indexOf(" ")), specifier: entry.slice(entry.indexOf(" ") + 1) })).filter(({ specifier }) => !BUILTINS.has(specifier));
  const opens = [...computed, ...unresolved];
  const projectJs = (program.projectJsDeclarations ?? 0) > 0;
  const openLoads = projectJs || opens.length > 0;
  const openProductLoads = projectJs || opens.some(({ file }) => fileClass(relative(file)) === "product");
  const hasUnreadLoaded = writers.some((writer) => writer.effective && writer.valueLoaded && writer.causes.some((cause) => cause.class.startsWith("unread:")));
  return {
    id: project.id,
    writers,
    openLoads,
    openProductLoads,
    opens: opens.slice(0, 5).map(({ file, specifier }) => `${relative(file)}${specifier ? ` ${specifier}` : ""}`),
    hasUnreadLoaded,
    rows: project.rows.filter(isFamily3).length,
    family3: project.rows.filter(isFamily3).map((reduced) => [rowKey(reduced), fileClass(relative(reduced[2]))]),
    census,
  };
}

/** Whether the mechanisms `S` (already closed over) decide the writer `writer` of `described`. */
export function decided(S, writer, described) {
  for (const id of S) {
    const mechanism = MECHANISMS[id];
    if (mechanism.level === "root" && mechanism.decides(writer, described)) return id;
  }
  if (writer.causeClasses.length === 0) return null;
  const by = new Set();
  for (const { cause, parsed } of writer.causeClasses) {
    let found = null;
    for (const id of S) {
      const mechanism = MECHANISMS[id];
      if (mechanism.level === "cause" && mechanism.decides(parsed, writer, cause, S)) {
        found = id;
        break;
      }
    }
    if (!found) return null;
    by.add(found);
  }
  return [...by].sort().join("+");
}

/** A row of family 3 as `[key, file class]`: the key names the site and the rule (`id|path@start-end`). */
export const rowKey = (reduced) => `${reduced[0]}|${reduced[2]}@${reduced[3]}-${reduced[4]}`;

/**
 * What the mechanisms `selected` do to one project: the writers left, and the rows that go if none is left. The rows that go are the baseline's family-3 rows the floor run
 * (every any-name writer left out) does not have (`floorKeys`, a Set of `rowKey`s; `null` when there is no floor run, and then nothing is counted). M2b (the product realm) frees
 * a row only where the product realm is the whole question: a writer that only M2b decides still applies to the rows of files the product's entries do not reach, so those rows
 * (test, config and script files by name) stay.
 */
export function evaluate(selected, described, floorKeys) {
  const S = closeOver(selected);
  const effective = described.writers.filter((writer) => writer.effective);
  const left = effective.filter((writer) => !decided(S, writer, described));
  const rows = floorKeys ? described.family3.filter(([key]) => !floorKeys.has(key)) : null;
  let freed = 0;
  let upper = 0;
  if (effective.length > 0 && left.length === 0 && rows) {
    upper = rows.length;
    const withoutRealm = new Set([...S].filter((id) => id !== "M2b"));
    const realmOnly = S.has("M2b") && effective.some((writer) => !decided(withoutRealm, writer, described));
    freed = realmOnly ? rows.filter(([, cls]) => cls === "product").length : rows.length;
  }
  return { effective: effective.length, decided: effective.length - left.length, left, freed, upper };
}

/** The roots the mechanisms decide in one project, for the checker's drop list: every counted writer they decide, a clear one too (a mechanism that decides a cause decides it everywhere). */
export function dropRoots(selected, described) {
  const S = closeOver(selected);
  return described.writers.filter((writer) => writer.counted && decided(S, writer, described)).map((writer) => writer.root);
}

function parseArguments(argv) {
  const [command, ...rest] = argv;
  const options = { baseline: null, floor: null, "except-last": null, out: null, mechanisms: "", "apps-root": "/Users/thomas/Documents/Github/solid-checker-rc21-frozen-v2/apps" };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index].replace(/^--/, "");
    if (!(key in options)) throw new Error(`unknown option ${rest[index]}`);
    if ((key === "floor" || key === "except-last") && command === "drops") options[key] = true;
    else options[key] = rest[(index += 1)];
  }
  return { command, options };
}

async function main() {
  const { command, options } = parseArguments(process.argv.slice(2));
  if (command === "drops") {
    const run = loadRun(resolve(options.baseline));
    mkdirSync(resolve(options.out), { recursive: true });
    const selected = options.mechanisms.split(",").filter(Boolean);
    for (const id of selected) if (!MECHANISMS[id]) throw new Error(`unknown mechanism ${id}`);
    let written = 0;
    for (const project of run.values()) {
      const described = describe(project, resolve(options["apps-root"]));
      let roots;
      if (options.floor === true) roots = described.writers.filter((writer) => writer.counted).map((writer) => writer.root);
      else if (options["except-last"] === true) {
        // V2: every counted writer but the last effective one (by root, the order the checker's index keeps them in): where deciding all the others frees nothing, the rows
        // are the last writer's alone.
        const effective = described.writers.filter((writer) => writer.effective).map((writer) => writer.root).sort();
        if (effective.length < 2) continue;
        roots = described.writers.filter((writer) => writer.counted && writer.root !== effective.at(-1)).map((writer) => writer.root);
      } else roots = dropRoots(selected, described);
      if (roots.length === 0) continue;
      writeFileSync(join(resolve(options.out), `${project.id}.drop`), `${roots.join("\n")}\n`);
      written += 1;
    }
    process.stderr.write(`${written} projects have a drop list\n`);
    return;
  }
  if (command === "crosscheck") {
    // The offline decision against the checker's own: a run of the class what-if switch (`SOLID_CHECKER_WHATIF_WRITER_CAUSES`, census on) lists, per root, whether it is
    // still counted; the offline step predicts which roots the same mechanisms leave out. Any difference is a bug in one of the two.
    const baseline = loadRun(resolve(options.baseline));
    const whatIf = loadRun(resolve(options.out));
    const selected = options.mechanisms.split(",").filter(Boolean);
    let projects = 0;
    let roots = 0;
    const differences = [];
    for (const project of baseline.values()) {
      const other = whatIf.get(project.id);
      if (!other?.census) continue;
      projects += 1;
      const described = describe(project, resolve(options["apps-root"]));
      const predicted = new Set(dropRoots(selected, described));
      for (const writer of other.census.writers) {
        roots += 1;
        const left = !writer.counted;
        if (left !== predicted.has(writer.root)) differences.push(`${project.id}: ${writer.name ?? writer.root} checker ${left ? "left it out" : "counted it"}, offline ${predicted.has(writer.root) ? "leaves it out" : "counts it"}`);
      }
    }
    console.log(`crosscheck ${selected.join("+")}: ${projects} projects, ${roots} roots, ${differences.length} differences`);
    for (const line of differences.slice(0, 30)) console.log(`  ${line}`);
    process.exitCode = differences.length > 0 ? 1 : 0;
    return;
  }
  throw new Error("usage: drops | crosscheck (the tables: scripts/writer-census-tables.mjs)");
}

if (import.meta.url === `file://${process.argv[1]}`) await main();
