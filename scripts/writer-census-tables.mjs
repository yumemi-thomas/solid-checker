#!/usr/bin/env node
// The tables of the writer-cause census (ADR 1131's M0): from a census run (`scripts/writer-census.mjs --census`) and the scenario runs of
// `scripts/writer-census-scenarios.mjs` (the floor, with every any-name writer left out, and the sets of mechanisms: a scenario is named by its mechanisms, sorted,
// joined with `+`). A set with no scenario run is answered by the offline step alone and marked so.
//
//   node scripts/writer-census-tables.mjs --baseline <run dir> --scenarios <scenario dir> --out <dir> [--apps-root <apps>] [--emit-scenarios <file>]
//
// `--emit-scenarios` writes the scenarios to run (every mechanism alone, the plan's steps and frontier) and stops. Otherwise writes `<out>/results.json` (everything the
// markdown says, as data), `<out>/writers.md` (every writer of every project and why) and `<out>/tables.md` (the tables of `M0-results.md`). Rows are the checker's
// family-3 rows (SC2002 uncertifiable under a package that writes a built-in's prototype).
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";

import { MECHANISMS, closeOver, decided, describe, dropRoots, evaluate, isFamily3, loadRun, rowKey } from "./writer-census-report.mjs";

const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => (index % 2 === 0 ? [...pairs, [value.replace(/^--/, ""), all[index + 1]]] : pairs), []));
const appsRoot = resolve(args["apps-root"] ?? "/Users/thomas/Documents/Github/solid-checker-rc21-frozen-v2/apps");
const baseline = loadRun(resolve(args.baseline));
const scenarioDir = args.scenarios ? resolve(args.scenarios) : null;
const out = resolve(args.out);
mkdirSync(out, { recursive: true });

/** A scenario's result file, `null` when it was not run. */
const readScenario = (name) => (scenarioDir && existsSync(join(scenarioDir, "scenarios", `${name}.json`)) ? JSON.parse(readFileSync(join(scenarioDir, "scenarios", `${name}.json`), "utf8")) : null);
/** The reduced rows of one project of a scenario (the baseline's where the scenario left out no root there). */
const scenarioRows = (id, found) => {
  if (!found?.key) return baseline.get(id).rows;
  return JSON.parse(gunzipSync(readFileSync(join(scenarioDir, "cache", id, found.key, "rows.json.gz"))).toString("utf8"));
};
const floorScenario = readScenario("floor");
const floor = new Map();
if (floorScenario) for (const found of floorScenario.projects) floor.set(found.id, { rows: scenarioRows(found.id, found) });

const PROGRAM = (file) => file.startsWith("/");
const appOf = (id) => id.split("__")[0];
const projectOf = (id) => id.split("__").slice(1).join("__");
const number = (value) => value.toLocaleString("en-US");
const numeric = (cell) => /^[+\-]?[\d,.]+( \/ [\d,.]+)?$|^n\/a$|^-$/.test(String(cell));
const table = (headers, rows) => [`| ${headers.join(" | ")} |`, `|${headers.map((_, index) => (index > 0 && rows.length > 0 && rows.every((row) => numeric(row[index])) ? "---:" : "---")).join("|")}|`, ...rows.map((row) => `| ${row.join(" | ")} |`)].join("\n");

const described = new Map([...baseline.values()].map((project) => [project.id, describe(project, appsRoot)]));
const floorRows = (id) => (floor.has(id) ? floor.get(id).rows.filter(isFamily3).length : null);
// `--assume-floor-zero` ranks the plan before the floor run exists (every row of a freed project goes); the tables then are not the final ones.
const floorKeys = (id) => (floor.has(id) ? new Set(floor.get(id).rows.filter(isFamily3).map(rowKey)) : args["assume-floor-zero"] ? new Set() : null);
const total = (list, pick) => list.reduce((sum, item) => sum + pick(item), 0);

const nameOf = (selected) => [...selected].sort().join("+");
const stats = { pairs: 0, agree: 0, disagree: [] };

/**
 * The rows of a set of mechanisms, project by project: measured where a scenario of that name ran (the baseline's family-3 rows the run lacks), else what the offline
 * step says. `upper` is every row that goes; `freed` is the rows that go if the product realm is the whole question where M2b decided a writer (see `evaluate`).
 */
function run(selected) {
  const scenario = readScenario(nameOf(selected));
  const perProject = [...described.values()].map((project) => {
    const offline = evaluate(selected, project, floorKeys(project.id));
    const found = scenario?.projects.find((item) => item.id === project.id);
    let { freed, upper } = offline;
    let measured = false;
    if (found) {
      measured = true;
      const after = new Set(scenarioRows(project.id, found).filter(isFamily3).map(rowKey));
      const gone = project.family3.filter(([key]) => !after.has(key));
      upper = gone.length;
      freed = offline.realmOnly ? gone.filter(([, cls]) => cls === "product").length : gone.length;
      stats.pairs += 1;
      if (floorKeys(project.id) && upper === offline.upper) stats.agree += 1;
      else if (floorKeys(project.id)) stats.disagree.push(`${nameOf(selected)} ${project.id}: measured ${upper}, offline ${offline.upper}`);
    }
    return { id: project.id, app: appOf(project.id), rows: project.rows, ...offline, freed, upper, measured, moved: found?.moved ?? null };
  });
  return { selected, name: nameOf(selected), measured: Boolean(scenario), perProject, freed: total(perProject, (item) => item.freed), upper: total(perProject, (item) => item.upper), projectsFreed: perProject.filter((item) => item.freed > 0).length };
}

const results = { baseline: {}, mechanisms: {}, plan: [], left: [] };
const projects = [...described.values()];
results.baseline = {
  projects: projects.length,
  rows: total(projects, (project) => project.rows),
  floorRows: floor.size > 0 ? total(projects, (project) => floorRows(project.id) ?? project.rows) : null,
  effectiveWriters: total(projects, (project) => project.writers.filter((writer) => writer.effective).length),
};

const sections = [];

// `--emit-scenarios <file> [--stage A|B]`: the scenarios to run. Stage A needs nothing but the census: the floor, all but the last writer (V2), every mechanism alone and
// the two runs of the checker's own class matcher. Stage B is the plan's sets (the chain, the frontier), which need the floor's rows to rank.
if (args["emit-scenarios"]) {
  const list = [];
  const seen = new Set();
  const add = (scenario) => {
    if (!seen.has(scenario.name)) {
      seen.add(scenario.name);
      list.push(scenario);
    }
  };
  if ((args.stage ?? "A") === "A") {
    add({ name: "floor", floor: true });
    add({ name: "v2", exceptLast: true });
    for (const id of Object.keys(MECHANISMS)) add({ name: id, mechanisms: [id] });
    add({ name: "whatif-M3u", classes: "escape:source/,escape:view/,escape:defaultView/", mechanisms: ["M3u"] });
    add({ name: "whatif-M4", classes: "generated:function-member:,generated:function-literal:empty,generated:function-literal:const", mechanisms: ["M4"] });
    add({ name: "whatif-unloaded", classes: "unloaded", mechanisms: ["M2flagRaw"] });
  }
  if ((args.stage ?? "A") === "A") {
    writeFileSync(resolve(args["emit-scenarios"]), `${JSON.stringify(list, null, 1)}\n`);
    process.stderr.write(`${list.length} scenarios (stage A) -> ${args["emit-scenarios"]}\n`);
    process.exit(0);
  }
}

// 1. Baseline per app.
{
  const byApp = new Map();
  for (const project of projects) {
    const entry = byApp.get(appOf(project.id)) ?? { app: appOf(project.id), projects: 0, rows: 0, floor: 0, writers: 0, maxWriters: 0, open: 0, openProduct: 0, unread: 0 };
    const effective = project.writers.filter((writer) => writer.effective);
    entry.projects += 1;
    entry.rows += project.rows;
    entry.floor += floorRows(project.id) ?? project.rows;
    entry.writers += effective.length;
    entry.maxWriters = Math.max(entry.maxWriters, effective.length);
    entry.open += project.openLoads ? 1 : 0;
    entry.openProduct += project.openProductLoads ? 1 : 0;
    entry.unread += project.hasUnreadLoaded ? 1 : 0;
    byApp.set(entry.app, entry);
  }
  const rows = [...byApp.values()].sort((a, b) => b.rows - a.rows).map((entry) => [entry.app, entry.projects, number(entry.rows), floor.size ? number(entry.floor) : "n/a", entry.maxWriters, `${entry.open} / ${entry.openProduct}`, entry.unread]);
  sections.push(`### Baseline: family-3 rows, the floor, and the any-name writers per app (measured)\n\n${table(["app", "projects", "family-3 rows", "rows with every writer decided (floor)", "most writers in a project", "projects with an open load (any file / product files)", "projects whose loaded writers include unread code"], rows)}\n\nTotal: ${number(results.baseline.rows)} rows${floor.size ? `, ${number(results.baseline.floorRows)} at the floor (${number(results.baseline.rows - results.baseline.floorRows)} are held by any-name writers)` : ""}.`);
  results.baseline.byApp = [...byApp.values()];
}

// 2. Causes: how many roots have each cause family, and how many have only it.
{
  const families = new Map();
  const bump = (key, writerId, only) => {
    const entry = families.get(key) ?? { key, roots: new Set(), only: new Set() };
    entry.roots.add(writerId);
    if (only) entry.only.add(writerId);
    families.set(key, entry);
  };
  const classOf = (parsed) => {
    switch (parsed.family) {
      case "escape": return parsed.route ? `route:${parsed.head}` : `escape:${parsed.head === "this" ? "this" : parsed.head === "document" ? "document" : "global-object name"}`;
      case "generated": return `generated:${parsed.kind.replace(/:.*$/, "")}`;
      case "handed-on": return "prototype handed on";
      case "key": return "prototype member written under a key nothing decides";
      case "pattern": return "prototype bound by a pattern nothing reads";
      case "unread": return `unread:${parsed.blocker}`;
      default: return "other";
    }
  };
  for (const project of projects) {
    for (const writer of project.writers.filter((item) => item.effective)) {
      const key = `${appOf(project.id)}::${writer.canonical || writer.root}`;
      const kinds = new Set(writer.causeClasses.map(({ parsed }) => classOf(parsed)));
      for (const kind of kinds) bump(kind, key, kinds.size === 1);
    }
  }
  const rows = [...families.values()].sort((a, b) => b.roots.size - a.roots.size).map((entry) => [entry.key, number(entry.roots.size), number(entry.only.size)]);
  sections.push(`### Why a root is an any-name writer: roots (per app, a root counted once) holding each class of cause (measured)\n\n${table(["cause class", "roots with it", "roots with only it"], rows)}`);
  results.causeClasses = rows;
}

// 2a. What the rows print against why a root is a writer: the walk's first problem (Q2, "may it reach Solid") per writer root, and how many have no other.
{
  const first = new Map();
  let onlyUnknown = 0;
  let withUnknown = 0;
  let roots = 0;
  const seen = new Set();
  for (const project of projects) {
    for (const writer of project.writers.filter((item) => item.effective)) {
      const key = `${appOf(project.id)}::${writer.canonical || writer.root}`;
      if (seen.has(key)) continue;
      seen.add(key);
      roots += 1;
      const blocker = /blocker: (\w+)/.exec(writer.reach ?? "")?.[1] ?? (writer.reach ?? "").replace(/\W.*$/, "");
      first.set(blocker, (first.get(blocker) ?? 0) + 1);
      const kinds = Object.keys(writer.problemKinds ?? {});
      if (kinds.includes("UnknownGlobal")) withUnknown += 1;
      if (kinds.length > 0 && kinds.every((kind) => kind === "UnknownGlobal")) onlyUnknown += 1;
    }
  }
  const rows = [...first.entries()].sort((a, b) => b[1] - a[1]).map(([blocker, count]) => [blocker, number(count)]);
  sections.push(`### What a row prints against why the root is a writer (measured; ${number(roots)} writer packages, per app)\n\nThe first problem of the walk (the verdict a row's message prints) is the answer to "may it reach Solid". It is not why the root writes under any name: no unknown global sets that. ${number(withUnknown)} of the ${number(roots)} writers hold an unknown global among their problems; ${number(onlyUnknown)} hold nothing else (the roots M6 would decide by itself, whatever their causes).\n\n${table(["first problem of the root", "writer packages"], rows)}`);
  results.firstProblem = rows;
  results.unknownGlobal = { roots, withUnknown, onlyUnknown };
}

// 2b. The causes in detail: occurrences (n) and the roots (counted once per app) that have them.
{
  const shapeClass = (parsed) => {
    const alias = /^alias\[(.*)\]$/.exec(parsed.shape);
    let base;
    if (alias) {
      const uses = alias[1] === "" ? [] : alias[1].split(",");
      const reads = ["member", "pattern", "inspected"];
      base = uses.every((use) => reads.includes(use)) ? "alias, every use a read of named globals (M1)" : uses.every((use) => [...reads, "computed"].includes(use)) ? "alias, reads and computed reads (M1w)" : uses.every((use) => [...reads, "computed", "coerced", "regexp-arg"].includes(use)) ? "alias, also coerced or given to RegExp (M3)" : "alias handed on or stored";
    } else if (parsed.shape === "alias-open") base = "alias not followed (reassigned, exported, script var)";
    else if (/^(argument|new):/.test(parsed.shape)) base = parsed.shape.startsWith("argument:RegExp") || parsed.shape.startsWith("new:RegExp") ? "given to RegExp" : "handed to a call";
    else if (parsed.shape.startsWith("member:")) base = parsed.shape === "member:valueOf" ? "member valueOf" : "member read off a fallback";
    else base = parsed.shape;
    return `${parsed.branch ? "(through ||/??/?:) " : ""}${base}`;
  };
  const aggregate = (pick, keyOf) => {
    const map = new Map();
    for (const project of projects) {
      for (const writer of project.writers.filter((item) => item.effective)) {
        for (const { cause, parsed } of writer.causeClasses) {
          const key = pick(parsed, cause);
          if (key === null) continue;
          const entry = map.get(key) ?? { key, n: 0, roots: new Set() };
          entry.n += cause.n;
          entry.roots.add(`${appOf(project.id)}::${writer.canonical || writer.root}`);
          map.set(key, entry);
        }
      }
    }
    return [...map.values()].sort((a, b) => b.roots.size - a.roots.size || b.n - a.n).map((entry) => [entry.key, number(entry.roots.size), number(entry.n)]);
  };
  const names = aggregate((parsed) => (parsed.family === "escape" && !parsed.route ? shapeClass(parsed) : null));
  const routes = aggregate((parsed, cause) => (parsed.family === "escape" && parsed.route ? `${parsed.head} ${PROGRAM(cause.file) ? "in a file of the program" : "in installed JavaScript"}: ${parsed.receiver ? `receiver ${parsed.receiver.replace(/^member:.*/, "member")}` : "no receiver"}` : null));
  const routeShapes = aggregate((parsed) => (parsed.family === "escape" && parsed.route ? shapeClass(parsed) : null));
  const generated = aggregate((parsed) => (parsed.family === "generated" ? parsed.kind.replace(/^function-member:.*/, "function-member:<name>") : null));
  const handed = aggregate((parsed) => (parsed.family === "handed-on" ? parsed.shape.replace(/^(argument|new):.*/, "$1:<callee>") : null));
  const unread = aggregate((parsed) => (parsed.family === "unread" ? parsed.blocker : null));
  const head = ["class", "roots (per app)", "occurrences"];
  sections.push(`### Causes in detail (measured)\n\n**The global object handed on by a name (\`window\`, \`globalThis\`, \`self\`, \`this\`…), by what is done with it**\n\n${table(head, names)}\n\n**\`source\`/\`view\`/\`defaultView\` routes, by where they are and what they were read off**\n\n${table(head, routes)}\n\n**Routes by what is done with the window**\n\n${table(head, routeShapes)}\n\n**Generated code, by kind**\n\n${table(head, generated)}\n\n**A prototype handed on, by where it goes**\n\n${table(head, handed)}\n\n**Code nobody read, by blocker**\n\n${table(head, unread)}`);
  results.detail = { names, routes, routeShapes, generated, handed, unread };
}

// 3. Each mechanism alone, one table per mechanism per project.
const alone = ["M2flag", "M2a", "M2b", "M2aO", "M2bO", "M1", "M1w", "M1u", "M3", "X1131", "M3u", "M4", "M4w", "M5b", "M5", "M5u", "M6", "M7v"];
const aloneRuns = new Map(alone.map((id) => [id, run([id])]));
{
  const head = alone.map((id) => {
    const result = aloneRuns.get(id);
    return [`**${id}**`, MECHANISMS[id].label, MECHANISMS[id].level, result.projectsFreed, number(result.freed) + (result.freed !== result.upper ? ` (${number(result.upper)} if every row)` : ""), `${number(total(result.perProject, (item) => item.packages - item.packagesLeft))}`];
  });
  sections.push(`### Each mechanism alone: rows freed (measured ceiling; see 'how rows are measured')\n\n${table(["mechanism", "what it decides", "level", "projects freed", "rows freed", "writer packages decided (of " + number(total(projects, (project) => new Set(project.writers.filter((writer) => writer.effective).map((writer) => writer.canonical || writer.root)).size)) + " package-in-project pairs)"], head)}`);
  for (const id of alone) {
    const result = aloneRuns.get(id);
    const rows = result.perProject.filter((item) => item.effective > 0).sort((a, b) => b.rows - a.rows).map((item) => [projectOf(item.id) ? `${item.app}/${projectOf(item.id)}` : item.app, item.packages, item.packages - item.packagesLeft, item.packagesLeft, number(item.rows), number(item.freed) + (item.freed !== item.upper ? ` (${number(item.upper)} if every row)` : "")]);
    sections.push(`#### ${id}: ${MECHANISMS[id].label}${result.measured ? " (measured)" : " (offline: no run of this set)"}\n\n${table(["project", "any-name writer packages", "decided", "left", "family-3 rows", "rows freed"], rows)}`);
  }
  results.mechanisms = Object.fromEntries(alone.map((id) => [id, { freed: aloneRuns.get(id).freed, projectsFreed: aloneRuns.get(id).projectsFreed, perProject: aloneRuns.get(id).perProject.map(({ left, ...rest }) => ({ ...rest, left: left.length })) }]));
}

// 4. What empties a project. Every choice of one variant of each family of mechanisms (the upper bounds included), with its cost; per project the cheapest choice that decides
// every one of its writers, with the M2b ruling and without; then the ranked plan: the sets that free the most rows per unit of cost, each on top of the last.
{
  const menu = [["-", "M1", "M1w", "M1u"], ["-", "M3", "X1131", "M3u"], ["-", "M4", "M4w"], ["-", "M5b", "M5", "M5u"], ["-", "M2a", "M2b", "M2aO", "M2bO"], ["-", "M6"], ["-", "M7v"]];
  const combos = [];
  const walk = (index, picked) => {
    if (index === menu.length) {
      const selected = picked.filter((id) => id !== "-");
      combos.push({ selected, cost: total(selected, (id) => MECHANISMS[id].size), ruling: selected.some((id) => MECHANISMS[id].ruling), bound: selected.some((id) => ["M1u", "M3u", "M5u", "M2aO", "M2bO"].includes(id)) });
      return;
    }
    for (const choice of menu[index]) walk(index + 1, [...picked, choice]);
  };
  walk(0, []);
  combos.sort((a, b) => a.cost - b.cost || a.selected.length - b.selected.length);
  const live = projects.filter((project) => project.writers.some((writer) => writer.effective));
  const cheapest = (project, allowed) => combos.find((combo) => allowed(combo) && evaluate(combo.selected, project, new Set()).left.length === 0) ?? null;
  const unlock = live.map((project) => ({
    id: project.id,
    app: appOf(project.id),
    rows: project.rows,
    packages: new Set(project.writers.filter((writer) => writer.effective).map((writer) => writer.canonical || writer.root)).size,
    noRuling: cheapest(project, (combo) => !combo.ruling),
    withRuling: cheapest(project, () => true),
  }));
  const setName = (combo) => (combo ? `${combo.selected.join(" + ") || "none"}${combo.bound ? " (a bound)" : ""} (${combo.cost})` : "no set");
  // Where nothing empties a project: what holds it, under every mechanism together.
  const everything = run(["M1u", "M3u", "M4w", "M5u", "M2b", "M6", "M7v"]);
  const heldBy = new Map(everything.perProject.map((item) => [item.id, item]));
  const rows = unlock.sort((a, b) => b.rows - a.rows).map((item) => [`${item.app}/${projectOf(item.id)}`, number(item.rows), item.packages, setName(item.noRuling), setName(item.withRuling), item.withRuling ? "-" : `${heldBy.get(item.id).packagesLeft}: ${heldBy.get(item.id).leftNames.slice(0, 5).join(", ")}${heldBy.get(item.id).leftNames.length > 5 ? ", …" : ""}`]);
  sections.push(`### What empties a project: the cheapest set of mechanisms that decides every writer of it (offline from the census; cost S=1, M=2 … is the design report's sizing, an estimate)\n\nThe menu: M1/M1w/M1u, M3/X1131/M3u, M4/M4w, M5b/M5/M5u, M2a/M2b/M2aO/M2bO, M6, M7v (one variant of each family or none; M…u and M2…O are upper bounds that decide more than the design can). \`no set\` means no choice of the menu empties the project.\n\n${table(["project", "family-3 rows", "writer packages", "cheapest set without the M2b ruling", "cheapest set with it", "held by (under every mechanism), when no set empties it"], rows)}`);
  results.unlock = unlock.map((item) => ({ id: item.id, rows: item.rows, packages: item.packages, noRuling: item.noRuling?.selected ?? null, withRuling: item.withRuling?.selected ?? null }));

  // The ranked plan: greedy over the unlocking sets of the projects, each on top of the selection so far (set cover with a cost).
  const buildChain = (allowRuling, allowBound) => {
    const allowed = (combo) => (allowRuling || !combo.ruling) && (allowBound || !combo.bound);
    const candidates = new Map();
    for (const item of live) {
      const combo = cheapest(projects.find((project) => project.id === item.id), allowed);
      if (combo) candidates.set(combo.selected.join("+"), combo);
    }
    const steps = [];
    let selection = [];
    let current = run([]);
    // The union of two sets, a mechanism another one in it includes (M1 in M1w) left out.
    const unionOf = (a, b) => {
      const out = new Set([...a, ...b]);
      for (const id of [...out]) if ([...out].some((other) => other !== id && (MECHANISMS[other].includes ?? []).includes(id))) out.delete(id);
      return [...out];
    };
    for (;;) {
      let best = null;
      for (const candidate of candidates.values()) {
        const union = unionOf(selection, candidate.selected);
        if (nameOf(union) === nameOf(selection)) continue;
        const result = run(union);
        const cost = total(union, (id) => MECHANISMS[id].size) - total(selection, (id) => MECHANISMS[id].size);
        const gain = result.freed - current.freed;
        const score = cost > 0 ? gain / cost : gain;
        if (gain > 0 && (!best || score > best.score || (score === best.score && cost < best.cost))) best = { union, result, cost, gain, score };
      }
      if (!best) break;
      const wasFreed = new Set(current.perProject.filter((item) => item.freed > 0).map((item) => item.id));
      steps.push({ added: best.union.filter((id) => !selection.includes(id)), selected: best.union, cost: total(best.union, (id) => MECHANISMS[id].size), freed: best.result.freed, projects: best.result.projectsFreed, gain: best.gain, newly: best.result.perProject.filter((item) => item.freed > 0 && !wasFreed.has(item.id)).map((item) => `${item.app}/${projectOf(item.id)}`) });
      selection = best.union;
      current = best.result;
    }
    return steps;
  };
  const chainTable = (steps) => (steps.length === 0 ? "No set of the menu empties any project." : table(["step", "adds (the set so far)", "cumulative cost", "projects freed", "rows freed (cumulative)", "rows gained", "projects newly freed"], steps.map((step, index) => [index + 1, `${step.added.join(" + ")} (${step.selected.join(" + ")})`, step.cost, step.projects, number(step.freed), number(step.gain), step.newly.length > 6 ? `${step.newly.slice(0, 6).join(", ")}, … (${step.newly.length})` : step.newly.join(", ")])));
  const chain = buildChain(true, false);
  const chainNoRuling = buildChain(false, false);
  const chainBounds = buildChain(true, true);
  sections.push(`### The ranked plan (cumulative: at each step the set that frees the most rows per unit of cost on top of the last; the upper bounds M…u and M2…O left out)\n\n${chainTable(chain)}\n\n**Without the M2b ruling** (tests count as entries):\n\n${chainTable(chainNoRuling)}\n\n**With the upper bounds allowed** (what no design can decide, M1u, M3u, M5u, the optimistic M2 forms):\n\n${chainTable(chainBounds)}`);
  results.chain = chain;
  results.chainNoRuling = chainNoRuling;
  results.chainBounds = chainBounds;
  results.plan = chain;
  results.planNoRuling = chainNoRuling;
  results.combos = combos.length;
  results.everything = { freed: everything.freed, projectsFreed: everything.projectsFreed };
  const leftRows = [];
  for (const item of everything.perProject.filter((entry) => entry.effective > 0 && entry.packagesLeft > 0).sort((a, b) => b.rows - a.rows)) {
    leftRows.push([`${item.app}/${projectOf(item.id)}`, number(item.rows), item.packagesLeft, item.leftNames.slice(0, 8).join(", ") + (item.leftNames.length > 8 ? ", …" : "")]);
  }
  sections.push(`### What stays under every mechanism together (M1u, M3u, M4w, M5u, M2b, M6, M7v), by project (measured causes)\n\n${table(["project", "family-3 rows", "writer packages left", "which (first 8)"], leftRows)}`);
  results.left = leftRows;
  // The sets to run for the plan and the unlock table.
  results.stageB = [...new Map([...chain, ...chainNoRuling, ...chainBounds, ...unlock.flatMap((item) => [item.noRuling, item.withRuling].filter(Boolean).map((combo) => ({ selected: combo.selected })))].map((item) => [nameOf(item.selected), item])).values()];
}

if (args["emit-scenarios"] && args.stage === "B") {
  const list = [];
  const seen = new Set([...Object.keys(MECHANISMS), "floor", "v2"]);
  const sets = [...(results.stageB ?? []), { selected: ["M1u", "M3u", "M4w", "M5u", "M6"] }, { selected: ["M1u", "M3u", "M4w", "M5u", "M6", "M7v"] }, { selected: ["M1u", "M2aO", "M3u", "M4w", "M5u", "M6", "M7v"] }, { selected: ["M1u", "M2bO", "M3u", "M4w", "M5u", "M6", "M7v"] }, { selected: ["M1u", "M2b", "M3u", "M4w", "M5u", "M6", "M7v"] }];
  for (const set of sets) {
    const mechanisms = set.selected;
    const name = nameOf(mechanisms);
    if (mechanisms.length === 0 || seen.has(name)) continue;
    seen.add(name);
    list.push({ name, mechanisms });
  }
  writeFileSync(resolve(args["emit-scenarios"]), `${JSON.stringify(list, null, 1)}\n`);
  process.stderr.write(`${list.length} scenarios (stage B) -> ${args["emit-scenarios"]}\n`);
  process.exit(0);
}

// The checker's own class matcher (`SOLID_CHECKER_WHATIF_WRITER_CAUSES`) against the offline step: the roots a run still counts, from its census, against the roots the offline
// step leaves counted for the same mechanisms.
{
  const lines = [];
  for (const name of ["whatif-M3u", "whatif-M4", "whatif-unloaded"]) {
    const scenario = readScenario(name);
    if (!scenario) continue;
    let roots = 0;
    const differences = [];
    for (const found of scenario.projects) {
      if (!found.stillCounted) continue;
      const info = described.get(found.id);
      const predicted = new Set(dropRoots(scenario.scenario.mechanisms, info));
      for (const writer of info.writers.filter((item) => item.counted)) {
        roots += 1;
        const left = !found.stillCounted.includes(writer.root);
        if (left !== predicted.has(writer.root)) differences.push(`${found.id}: ${writer.name} (checker ${left ? "left it out" : "counted it"}, offline ${predicted.has(writer.root) ? "leaves it out" : "counts it"})`);
      }
    }
    lines.push(`- \`${name}\` (\`${scenario.scenario.classes}\`): ${scenario.projects.filter((found) => found.stillCounted).length} projects, ${number(roots)} writer roots compared, ${differences.length} differences${differences.length ? `: ${differences.slice(0, 6).join("; ")}` : ""}`);
    results.classMatcher = { ...(results.classMatcher ?? {}), [name]: { roots, differences } };
  }
  if (lines.length) sections.push(`### The checker's own class matcher against the offline step (measured)\n\n${lines.join("\n")}`);
}

sections.push(`### Offline prediction against the runs (measured)\n\nThe offline step decides, from the census, which roots a set of mechanisms leaves out and so whether a project's rows go; the checker then ran each set with those roots left out. The rows that went agreed with the offline prediction in ${stats.agree} of ${stats.agree + stats.disagree.length} (scenario, project) pairs${stats.disagree.length ? `; the others: ${stats.disagree.slice(0, 12).join("; ")}${stats.disagree.length > 12 ? ", …" : ""}` : ""}.`);
results.agreement = { pairs: stats.pairs, agree: stats.agree, disagree: stats.disagree };

writeFileSync(join(out, "tables.md"), `${sections.join("\n\n")}\n`);

// The writers of every project and why. Projects of one app that share the same writers (kui's eleven) are listed once.
{
  const lines = ["# Writers by project (measured: census of the any-name prototype writers of the 34-project corpus)", "", "A writer is a root the checker counts as one that may write a built-in's prototype under any name and that is not clear (its closure may reach Solid). `loaded`: the backend's own flag; `value`/`product`: loaded by a value import of a project file / of a product file (the file-name test of `scripts/writer-census-report.mjs`), directly or through a package that does; `loaded by`: the file classes of the project files that import it by value. `first problem`: what the row's message prints (the walk's first problem in order, which is not why the root is a writer). `signature`: the cause families, `esc` (the global object handed on by a name: `window`, `globalThis`, `self`, `this`…), `route` (`x.source`, `x.view`, `x.defaultView` taken for the window), `gen` (code generated at run time), `handed` (a built-in's prototype handed on), `key` (a prototype member written under a key nothing decides), `pat` (a prototype bound by a pattern nothing reads), `unread` (code nobody read: the blocker). Causes are `class×count` per class, most frequent first.", ""];
  const familyOf = (parsed) => (parsed.family === "escape" ? (parsed.route ? "route" : "esc") : parsed.family === "generated" ? "gen" : parsed.family === "handed-on" ? "handed" : parsed.family === "key" ? "key" : parsed.family === "pattern" ? "pat" : parsed.family === "unread" ? `unread(${parsed.blocker})` : "other");
  const everyMechanism = closeOver(["M1u", "M3u", "M4w", "M5u", "M2b", "M6", "M7v"]);
  const causeLevel = closeOver(["M1u", "M3u", "M4w", "M5u"]);
  const rowOf = (writer, project) => {
    // What would decide it: the mechanisms that decide the root whole or all of its causes together, else the causes that stay (unread code, real hand-ons, generation).
    const by = decided(everyMechanism, writer, project);
    const hard = new Map();
    for (const { cause, parsed } of writer.causeClasses) {
      if (parsed.family === "unread" || !MECHANISMS.M1u.decides(parsed) && !MECHANISMS.M3u.decides(parsed) && !MECHANISMS.M5u.decides(parsed) && !MECHANISMS.M4w.decides(parsed, writer, cause, causeLevel)) hard.set(familyOf(parsed), (hard.get(familyOf(parsed)) ?? 0) + cause.n);
      else if (parsed.family === "generated" && !MECHANISMS.M4w.decides(parsed, writer, cause, causeLevel)) hard.set(familyOf(parsed), (hard.get(familyOf(parsed)) ?? 0) + cause.n);
    }
    const stays = [...hard.entries()].sort((a, b) => b[1] - a[1]).map(([family, count]) => `${family}×${count}`).join(" ");
    const byClass = new Map();
    for (const cause of writer.causes) byClass.set(cause.class, (byClass.get(cause.class) ?? 0) + cause.n);
    const families = new Map();
    for (const { cause, parsed } of writer.causeClasses) families.set(familyOf(parsed), (families.get(familyOf(parsed)) ?? 0) + cause.n);
    const signature = [...families.entries()].sort((a, b) => b[1] - a[1]).map(([family, count]) => `${family}×${count}`).join(" ");
    const causes = [...byClass.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5).map(([cls, count]) => `${cls}×${count}`).join("; ") + (byClass.size > 5 ? `; … (${byClass.size} classes)` : "");
    const files = [...new Set(writer.causes.filter((cause) => cause.file).map((cause) => cause.file))].slice(0, 2).join(", ");
    return [writer.name, writer.loaded ? "y" : "n", writer.valueLoaded ? "y" : "n", writer.productLoaded ? "y" : "n", writer.importerClasses.join("+") || "none", by ? `${by}` : `stays: ${stays || "-"}`, (writer.reach ?? "").replace(/^Undecided \{ blocker: (\w+), detail: "([^"]{0,60}).*$/s, "$1 $2").replace(/^ReachesSolid.*$/s, "ReachesSolid"), signature, causes, files];
  };
  const groups = new Map();
  for (const project of projects.sort((a, b) => (a.id < b.id ? -1 : 1))) {
    const writers = project.writers.filter((writer) => writer.effective);
    if (writers.length === 0) continue;
    const rows = writers.map((writer) => rowOf(writer, project));
    const key = `${appOf(project.id)}\n${JSON.stringify(rows)}`;
    const group = groups.get(key) ?? { app: appOf(project.id), projects: [], rows, writers: writers.length, open: project.openLoads };
    group.projects.push({ project: projectOf(project.id), rows: project.rows });
    groups.set(key, group);
  }
  for (const group of groups.values()) {
    const names = group.projects.map((entry) => `${entry.project} (${number(entry.rows)})`).join(", ");
    lines.push(`## ${group.app}: ${group.writers} writers; ${names}${group.open ? "; the project has an open load (computed or unresolved)" : ""}`, "");
    lines.push(table(["root", "loaded", "value", "product", "loaded by", "decided by (every mechanism) or what stays", "first problem", "signature", "causes (class×count)", "files (first 2)"], group.rows), "");
  }
  writeFileSync(join(out, "writers.md"), `${lines.join("\n")}\n`);
}
writeFileSync(join(out, "results.json"), `${JSON.stringify(results, null, 1)}\n`);
process.stderr.write(`wrote ${join(out, "tables.md")}, writers.md, results.json\n`);
