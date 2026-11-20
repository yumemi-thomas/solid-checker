#!/usr/bin/env node
// Writer-cause scenarios (ADR 1131's M0, measurement only): runs the checker over the corpus with the what-if switch (`SOLID_CHECKER_WHATIF_WRITER_DROP`,
// `SOLID_CHECKER_WHATIF_WRITER_CAUSES`) for each scenario of a list, and keeps what moved. A scenario decides a set of mechanisms (`scripts/writer-census-report.mjs`
// `MECHANISMS`): the roots they decide in a project, from its baseline census, are left out of the any-name writers, and the project's rows are measured with the checker.
//
//   node scripts/writer-census-scenarios.mjs --baseline <census run dir> --out <dir> --checker <bin> --typefacts <bin> --scenarios <file.json> [--jobs 4]
//
// A scenario is `{ "name", "mechanisms": ["M1", …] }`, `{ "name", "floor": true }` (every counted writer left out), `{ "name", "exceptLast": true }` (all but the last
// effective writer), or with `"classes": "<prefix,…,unloaded>"` a run of the checker's own class matcher (the mechanisms then say what the offline step predicts, which
// `--check` compares with the roots the checker still counts). A project whose scenario leaves out no root is the baseline's (no run). Runs are kept by project and drop
// set under `<out>/cache`, so a drop set that several scenarios share runs once. Writes `<out>/scenarios/<name>.json`: per project the rows after, and what else moved.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";

import { DEFAULT_APPS_ROOT } from "./lib/corpus-manifest.mjs";
import { isFamily3, runProject, saveResult } from "./writer-census.mjs";
import { describe, dropRoots, loadRun } from "./writer-census-report.mjs";

function parseArguments(argv) {
  const options = { baseline: null, out: null, checker: null, typefacts: null, scenarios: null, jobs: "4", timeout: "900", manifest: null, "apps-root": DEFAULT_APPS_ROOT, only: null };
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index].replace(/^--/, "");
    if (!(key in options) || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
    options[key] = argv[index + 1];
  }
  for (const key of ["baseline", "out", "checker", "typefacts", "scenarios"]) if (!options[key]) throw new Error(`--${key} is required`);
  return options;
}

/** The rows of a run that moved against the baseline: by rule, how many left, arrived or changed kind. */
function moved(baseRows, rows) {
  const key = (row) => `${row[0]}|${row[2]}@${row[3]}-${row[4]}|${row[5]}`;
  const was = new Map();
  for (const row of baseRows) was.set(key(row), row[1]);
  const now = new Map();
  for (const row of rows) now.set(key(row), row[1]);
  const byRule = {};
  const bump = (rule, name) => {
    byRule[rule] ??= { gone: 0, arrived: 0, changed: 0 };
    byRule[rule][name] += 1;
  };
  for (const [row, kind] of was) {
    if (!now.has(row)) bump(row.split("|")[0], "gone");
    else if (now.get(row) !== kind) bump(row.split("|")[0], "changed");
  }
  for (const row of now.keys()) if (!was.has(row)) bump(row.split("|")[0], "arrived");
  const violations = rows.filter((row) => row[1] === "violation").length - baseRows.filter((row) => row[1] === "violation").length;
  return { byRule, violationsDelta: violations };
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const appsRoot = resolve(options["apps-root"]);
  const manifest = JSON.parse(readFileSync(resolve(options.manifest ?? join(resolve(new URL("..", import.meta.url).pathname), "scripts/corpus/rc21-manifest.json")), "utf8"));
  const rowsOf = new Map(manifest.projects.map((row) => [`${row.app}__${row.project.replace(/[^A-Za-z0-9.-]+/g, "_")}`, { app: row.app, project: row.project, root: join(appsRoot, row.root) }]));
  const baseline = loadRun(resolve(options.baseline));
  const described = new Map([...baseline.values()].map((project) => [project.id, describe(project, appsRoot)]));
  const scenarios = JSON.parse(readFileSync(resolve(options.scenarios), "utf8")).filter((scenario) => !options.only || options.only.split(",").includes(scenario.name));
  const out = resolve(options.out);
  mkdirSync(join(out, "scenarios"), { recursive: true });
  const runOptions = { checker: options.checker, typefacts: options.typefacts, timeout: options.timeout, census: true, "keep-report": false };

  // What each scenario leaves out of each project.
  const plan = [];
  const tasks = new Map();
  for (const scenario of scenarios) {
    const perProject = [];
    for (const project of baseline.values()) {
      const info = described.get(project.id);
      const counted = info.writers.filter((writer) => writer.counted);
      let roots;
      if (scenario.floor) roots = counted.map((writer) => writer.root);
      else if (scenario.exceptLast) {
        const effective = info.writers.filter((writer) => writer.effective).map((writer) => writer.root).sort();
        roots = effective.length < 2 ? [] : counted.filter((writer) => writer.root !== effective.at(-1)).map((writer) => writer.root);
      } else roots = dropRoots(scenario.mechanisms ?? [], info);
      const classes = scenario.classes ?? null;
      const runs = classes ? counted.length > 0 : roots.length > 0;
      const key = runs ? createHash("sha1").update(classes ? `classes:${classes}` : roots.slice().sort().join("\n")).digest("hex").slice(0, 16) : null;
      if (runs && !tasks.has(`${project.id}/${key}`)) tasks.set(`${project.id}/${key}`, { id: project.id, key, roots, classes });
      perProject.push({ id: project.id, key, dropped: roots.length, roots });
    }
    plan.push({ scenario, perProject });
  }

  // Run what has not been run.
  const todo = [...tasks.values()].filter((task) => !existsSync(join(out, "cache", task.id, task.key, "meta.json")));
  process.stderr.write(`${scenarios.length} scenarios, ${tasks.size} distinct runs, ${todo.length} to run\n`);
  let next = 0;
  let finished = 0;
  const worker = async () => {
    while (next < todo.length) {
      const task = todo[next++];
      const row = rowsOf.get(task.id);
      const directory = join(out, "cache", task.id, task.key);
      mkdirSync(directory, { recursive: true });
      const perRun = { ...runOptions };
      if (task.classes) perRun["whatif-causes"] = task.classes;
      else {
        // The driver reads a drop file named `<id>.drop` in the drop directory.
        writeFileSync(join(directory, `${task.id}.drop`), `${task.roots.join("\n")}\n`);
        perRun["drop-dir"] = directory;
      }
      const result = await runProject(perRun, manifest.configuration, row, task.id);
      saveResult(directory, perRun, row, task.id, result, appsRoot);
      finished += 1;
      process.stderr.write(`${finished}/${todo.length} ${task.id} ${task.key} exit=${result.exitCode} ${result.wallMs}ms\n`);
    }
  };
  await Promise.all(Array.from({ length: Number(options.jobs) }, worker));

  // Assemble.
  for (const { scenario, perProject } of plan) {
    const projects = [];
    for (const item of perProject) {
      const base = baseline.get(item.id);
      let after = { meta: base.meta, rows: base.rows, census: base.census };
      if (item.key) {
        const directory = join(out, "cache", item.id, item.key);
        const meta = JSON.parse(readFileSync(join(directory, "meta.json"), "utf8"));
        const rows = JSON.parse(gunzipSync(readFileSync(join(directory, "rows.json.gz"))).toString("utf8"));
        const census = existsSync(join(directory, "census.json")) ? JSON.parse(readFileSync(join(directory, "census.json"), "utf8")) : null;
        after = { meta, rows, census };
      }
      const family3 = after.rows.filter(isFamily3).length;
      projects.push({
        id: item.id,
        dropped: item.dropped,
        key: item.key,
        exitCode: after.meta.exitCode,
        family3,
        family3Base: base.rows.filter(isFamily3).length,
        uncertifiable: after.meta.uncertifiable,
        violations: after.meta.violations,
        moved: item.key ? moved(base.rows, after.rows) : { byRule: {}, violationsDelta: 0 },
        stillCounted: after.census ? after.census.writers.filter((writer) => writer.counted).map((writer) => writer.root) : null,
      });
    }
    const totals = {
      family3: projects.reduce((sum, project) => sum + project.family3, 0),
      family3Base: projects.reduce((sum, project) => sum + project.family3Base, 0),
      freed: projects.reduce((sum, project) => sum + (project.family3Base - project.family3), 0),
      projectsChanged: projects.filter((project) => project.family3 !== project.family3Base).length,
      violationsDelta: projects.reduce((sum, project) => sum + project.moved.violationsDelta, 0),
      errors: projects.filter((project) => project.exitCode !== 0).length,
    };
    writeFileSync(join(out, "scenarios", `${scenario.name}.json`), `${JSON.stringify({ scenario, totals, projects }, null, 1)}\n`);
    process.stdout.write(`${scenario.name.padEnd(28)} rows ${String(totals.family3).padStart(8)}  freed ${String(totals.freed).padStart(8)}  projects changed ${String(totals.projectsChanged).padStart(3)}  violations ${totals.violationsDelta >= 0 ? "+" : ""}${totals.violationsDelta}  errors ${totals.errors}\n`);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) await main();
