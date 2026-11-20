#!/usr/bin/env node
// The tables of the writer-cause census (ADR 1131's M0): from a census run (`scripts/writer-census.mjs --census`) and, for the rows a mechanism frees, a floor run (the
// same corpus with every any-name writer left out: `scripts/writer-census-report.mjs drops --floor`, then `scripts/writer-census.mjs --drop-dir`).
//
//   node scripts/writer-census-tables.mjs --baseline <run dir> --floor <run dir> --out <dir> [--apps-root <apps>]
//
// Writes `<out>/results.json` (everything the markdown says, as data), `<out>/writers.md` (every writer of every project and why) and `<out>/tables.md` (the tables of
// `M0-results.md`). Rows are the checker's family-3 rows (SC2002 uncertifiable under a package that writes a built-in's prototype).
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { MECHANISMS, closeOver, decided, describe, evaluate, isFamily3, loadRun, rowKey } from "./writer-census-report.mjs";

const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => (index % 2 === 0 ? [...pairs, [value.replace(/^--/, ""), all[index + 1]]] : pairs), []));
const appsRoot = resolve(args["apps-root"] ?? "/Users/thomas/Documents/Github/solid-checker-rc21-frozen-v2/apps");
const baseline = loadRun(resolve(args.baseline));
const floor = args.floor ? loadRun(resolve(args.floor)) : new Map();
const out = resolve(args.out);
mkdirSync(out, { recursive: true });

const PROGRAM = (file) => file.startsWith("/");
const appOf = (id) => id.split("__")[0];
const projectOf = (id) => id.split("__").slice(1).join("__");
const number = (value) => value.toLocaleString("en-US");
const numeric = (cell) => /^[+\-]?[\d,.]+( \/ [\d,.]+)?$|^n\/a$|^-$/.test(String(cell));
const table = (headers, rows) => [`| ${headers.join(" | ")} |`, `|${headers.map((_, index) => (index > 0 && rows.length > 0 && rows.every((row) => numeric(row[index])) ? "---:" : "---")).join("|")}|`, ...rows.map((row) => `| ${row.join(" | ")} |`)].join("\n");

const described = new Map([...baseline.values()].map((project) => [project.id, describe(project, appsRoot)]));
const floorRows = (id) => (floor.has(id) ? floor.get(id).rows.filter(isFamily3).length : null);
const floorKeys = (id) => (floor.has(id) ? new Set(floor.get(id).rows.filter(isFamily3).map(rowKey)) : null);
const total = (list, pick) => list.reduce((sum, item) => sum + pick(item), 0);

/** The rows of a set of mechanisms, project by project. */
function run(selected) {
  const perProject = [...described.values()].map((project) => ({ id: project.id, app: appOf(project.id), rows: project.rows, ...evaluate(selected, project, floorKeys(project.id)) }));
  return { selected, perProject, freed: total(perProject, (item) => item.freed), upper: total(perProject, (item) => item.upper), projectsFreed: perProject.filter((item) => item.freed > 0).length };
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
const alone = ["M2flag", "M2a", "M2b", "M1", "M1w", "M1u", "M3", "X1131", "M3u", "M4", "M4w", "M5", "M6", "M7v"];
const aloneRuns = new Map(alone.map((id) => [id, run([id])]));
{
  const head = alone.map((id) => {
    const result = aloneRuns.get(id);
    return [`**${id}**`, MECHANISMS[id].label, MECHANISMS[id].level, result.projectsFreed, number(result.freed), number(total(result.perProject, (item) => item.decided))];
  });
  sections.push(`### Each mechanism alone: rows freed (measured ceiling; see 'how rows are measured')\n\n${table(["mechanism", "what it decides", "level", "projects freed", "rows freed", "writers decided (of " + number(results.baseline.effectiveWriters) + ")"], head)}`);
  for (const id of alone) {
    const result = aloneRuns.get(id);
    const rows = result.perProject.filter((item) => item.effective > 0).sort((a, b) => b.rows - a.rows).map((item) => [projectOf(item.id) ? `${item.app}/${projectOf(item.id)}` : item.app, item.effective, item.decided, item.left.length, number(item.rows), number(item.freed)]);
    sections.push(`#### ${id}: ${MECHANISMS[id].label}\n\n${table(["project", "any-name writers", "decided", "left", "family-3 rows", "rows freed"], rows)}`);
  }
  results.mechanisms = Object.fromEntries(alone.map((id) => [id, { freed: aloneRuns.get(id).freed, projectsFreed: aloneRuns.get(id).projectsFreed, perProject: aloneRuns.get(id).perProject.map(({ left, ...rest }) => ({ ...rest, left: left.length })) }]));
}

// 4. Combinations: every choice of one variant of each family, with its cost; the frontier.
{
  const choices = [["-", "M1", "M1w"], ["-", "M3"], ["-", "M4", "M4w"], ["-", "M5"], ["-", "M2a", "M2b"], ["-", "M6"], ["-", "M7v"]];
  const combos = [];
  const walk = (index, picked) => {
    if (index === choices.length) {
      const selected = picked.filter((id) => id !== "-");
      const result = run(selected);
      combos.push({ selected, cost: total(selected, (id) => MECHANISMS[id].size), freed: result.freed, projects: result.projectsFreed, ruling: selected.some((id) => MECHANISMS[id].ruling) });
      return;
    }
    for (const choice of choices[index]) walk(index + 1, [...picked, choice]);
  };
  walk(0, []);
  combos.sort((a, b) => a.cost - b.cost || b.freed - a.freed);
  const frontier = [];
  let best = -1;
  for (const combo of combos) {
    if (combo.freed > best) {
      frontier.push(combo);
      best = combo.freed;
    }
  }
  const noRuling = [];
  best = -1;
  for (const combo of combos.filter((item) => !item.ruling)) {
    if (combo.freed > best) {
      noRuling.push(combo);
      best = combo.freed;
    }
  }
  const show = (list) => table(["mechanisms", "cost (S=1, M=2 …)", "projects freed", "rows freed", "needs the M2b ruling"], list.map((combo) => [combo.selected.join(" + ") || "none", combo.cost, combo.projects, number(combo.freed), combo.ruling ? "yes" : "no"]));
  sections.push(`### Cumulative ranked plan: the cheapest set of mechanisms for each gain (frontier over all ${combos.length} choices; cost is the design report's S/M sizing, an estimate)\n\n${show(frontier)}\n\nWithout the M2b ruling (tests count as entries):\n\n${show(noRuling)}`);
  results.plan = frontier;
  results.planNoRuling = noRuling;
  results.combos = combos.length;
  // The chain: one ranked order of building, each step the upgrade (or up to three together, where one alone frees nothing: the last writer of a project has to go) with
  // the most rows per unit of cost; every step is a superset of the one before.
  const families = [["M1", "M1w"], ["M3"], ["M4", "M4w"], ["M5"], ["M2a", "M2b"], ["M6"], ["M7v"]];
  const buildChain = (allowed) => {
    const levels = families.map(() => 0);
    const picked = () => families.flatMap((family, index) => family.slice(0, levels[index]).slice(-1)).filter((id) => allowed(id));
    const levelOf = (family, level) => (family.slice(0, level).filter((id) => allowed(id)).at(-1) ? level : 0);
    const steps = [];
    let current = run(picked());
    const costOf = (list) => total(list, (id) => MECHANISMS[id].size);
    for (;;) {
      const upgrades = [];
      families.forEach((family, index) => {
        for (let level = levels[index] + 1; level <= family.length; level += 1) if (allowed(family[level - 1])) upgrades.push({ index, level });
      });
      if (upgrades.length === 0) break;
      let best = null;
      const consider = (bundle) => {
        const trial = levels.slice();
        for (const { index, level } of bundle) trial[index] = Math.max(trial[index], level);
        const ids = families.flatMap((family, index) => family.slice(0, trial[index]).slice(-1)).filter((id) => allowed(id));
        const result = run(ids);
        const cost = costOf(ids) - costOf(picked());
        const gain = result.freed - current.freed;
        const score = cost > 0 ? gain / cost : gain;
        if (!best || score > best.score || (score === best.score && cost < best.cost)) best = { bundle, trial, ids, result, cost, gain, score };
      };
      for (const first of upgrades) {
        consider([first]);
        for (const second of upgrades) if (second.index > first.index) {
          consider([first, second]);
          for (const third of upgrades) if (third.index > second.index) consider([first, second, third]);
        }
      }
      // Where nothing gains, take the cheapest single upgrade so that the chain goes on.
      if (best.gain === 0) {
        best = null;
        for (const upgrade of upgrades) {
          const before = best;
          consider([upgrade]);
          if (before && best.cost > before.cost) best = before;
        }
      }
      const addedIds = best.bundle.map(({ index, level }) => families[index][level - 1]);
      const wasFreed = new Set(current.perProject.filter((item) => item.freed > 0).map((item) => item.id));
      steps.push({ added: addedIds, selected: best.ids, cost: costOf(best.ids), freed: best.result.freed, projects: best.result.projectsFreed, gain: best.gain, newly: best.result.perProject.filter((item) => item.freed > 0 && !wasFreed.has(item.id)).map((item) => `${item.app}/${projectOf(item.id)}`) });
      best.trial.forEach((level, index) => { levels[index] = level; });
      current = best.result;
    }
    return steps;
  };
  const chainTable = (steps) => table(["step", "adds", "cumulative cost", "projects freed", "rows freed (cumulative)", "rows gained", "projects newly freed"], steps.map((step, index) => [index + 1, step.added.join(" + "), step.cost, step.projects, number(step.freed), number(step.gain), step.newly.length === 0 ? "-" : step.newly.length > 6 ? `${step.newly.slice(0, 6).join(", ")}, … (${step.newly.length})` : step.newly.join(", ")]));
  const chain = buildChain(() => true);
  const chainNoRuling = buildChain((id) => !MECHANISMS[id].ruling);
  sections.push(`### The ranked build order (cumulative; a step is the upgrade, or up to three together, with the most rows per unit of cost)\n\n${chainTable(chain)}\n\nWithout the M2b ruling:\n\n${chainTable(chainNoRuling)}`);
  results.chain = chain;
  results.chainNoRuling = chainNoRuling;
  const everything = run(["M1u", "M3u", "M4w", "M5", "M2b", "M6", "M7v"]);
  results.everything = { freed: everything.freed, projectsFreed: everything.projectsFreed };
  // What holds the rows the full set cannot free.
  const rows = [];
  for (const item of everything.perProject.filter((entry) => entry.effective > 0 && entry.left.length > 0).sort((a, b) => b.rows - a.rows)) {
    const writers = item.left.map((writer) => `${writer.name}`).slice(0, 8).join(", ");
    rows.push([`${item.app}/${projectOf(item.id)}`, number(item.rows), item.left.length, writers + (item.left.length > 8 ? ", …" : "")]);
  }
  sections.push(`### What stays under every mechanism together (M1u, M3u, M4w, M5, M2b, M6, M7v), by project (measured causes)\n\n${table(["project", "family-3 rows", "writers left", "which (first 8)"], rows)}`);
  results.left = rows;
}

writeFileSync(join(out, "tables.md"), `${sections.join("\n\n")}\n`);

// The writers of every project and why. Projects of one app that share the same writers (kui's eleven) are listed once.
{
  const lines = ["# Writers by project (measured: census of the any-name prototype writers of the 34-project corpus)", "", "A writer is a root the checker counts as one that may write a built-in's prototype under any name and that is not clear (its closure may reach Solid). `loaded`: the backend's own flag; `value`/`product`: loaded by a value import of a project file / of a product file (the file-name test of `scripts/writer-census-report.mjs`), directly or through a package that does; `loaded by`: the file classes of the project files that import it by value. `first problem`: what the row's message prints (the walk's first problem in order, which is not why the root is a writer). `signature`: the cause families, `esc` (the global object handed on by a name: `window`, `globalThis`, `self`, `this`…), `route` (`x.source`, `x.view`, `x.defaultView` taken for the window), `gen` (code generated at run time), `handed` (a built-in's prototype handed on), `key` (a prototype member written under a key nothing decides), `pat` (a prototype bound by a pattern nothing reads), `unread` (code nobody read: the blocker). Causes are `class×count` per class, most frequent first.", ""];
  const familyOf = (parsed) => (parsed.family === "escape" ? (parsed.route ? "route" : "esc") : parsed.family === "generated" ? "gen" : parsed.family === "handed-on" ? "handed" : parsed.family === "key" ? "key" : parsed.family === "pattern" ? "pat" : parsed.family === "unread" ? `unread(${parsed.blocker})` : "other");
  const rowOf = (writer) => {
    const byClass = new Map();
    for (const cause of writer.causes) byClass.set(cause.class, (byClass.get(cause.class) ?? 0) + cause.n);
    const families = new Map();
    for (const { cause, parsed } of writer.causeClasses) families.set(familyOf(parsed), (families.get(familyOf(parsed)) ?? 0) + cause.n);
    const signature = [...families.entries()].sort((a, b) => b[1] - a[1]).map(([family, count]) => `${family}×${count}`).join(" ");
    const causes = [...byClass.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5).map(([cls, count]) => `${cls}×${count}`).join("; ") + (byClass.size > 5 ? `; … (${byClass.size} classes)` : "");
    const files = [...new Set(writer.causes.filter((cause) => cause.file).map((cause) => cause.file))].slice(0, 2).join(", ");
    return [writer.name, writer.loaded ? "y" : "n", writer.valueLoaded ? "y" : "n", writer.productLoaded ? "y" : "n", writer.importerClasses.join("+") || "none", (writer.reach ?? "").replace(/^Undecided \{ blocker: (\w+), detail: "([^"]{0,60}).*$/s, "$1 $2").replace(/^ReachesSolid.*$/s, "ReachesSolid"), signature, causes, files];
  };
  const groups = new Map();
  for (const project of projects.sort((a, b) => (a.id < b.id ? -1 : 1))) {
    const writers = project.writers.filter((writer) => writer.effective);
    if (writers.length === 0) continue;
    const rows = writers.map(rowOf);
    const key = `${appOf(project.id)}\n${JSON.stringify(rows)}`;
    const group = groups.get(key) ?? { app: appOf(project.id), projects: [], rows, writers: writers.length, open: project.openLoads };
    group.projects.push({ project: projectOf(project.id), rows: project.rows });
    groups.set(key, group);
  }
  for (const group of groups.values()) {
    const names = group.projects.map((entry) => `${entry.project} (${number(entry.rows)})`).join(", ");
    lines.push(`## ${group.app}: ${group.writers} writers; ${names}${group.open ? "; the project has an open load (computed or unresolved)" : ""}`, "");
    lines.push(table(["root", "loaded", "value", "product", "loaded by", "first problem", "signature", "causes (class×count)", "files (first 2)"], group.rows), "");
  }
  writeFileSync(join(out, "writers.md"), `${lines.join("\n")}\n`);
}
writeFileSync(join(out, "results.json"), `${JSON.stringify(results, null, 1)}\n`);
process.stderr.write(`wrote ${join(out, "tables.md")}, writers.md, results.json\n`);
