// Emits a program as an oracle case directory, so Chrome confirmation is just
// `scripts/runtime-oracle.mjs --cases <parent> --only <id>`.
//
//   App.tsx        the clean program (every exported capitalized function is rendered by the oracle);
//                  what the checker reads and what an owner reads in a repro
//   oracle.json    { note, steps }: the driving plan as scenario steps (click each plan button,
//                  then read each witness selector so the report keeps its text)
//   App.run.txt    the instrumented twin (hits.mjs); run as App.tsx by the fast path and by Chrome
//   oracle.run.json  the twin's steps: the same, then two reads of #sc-hits (the hit readout)
//   program.json   the construct tree (what the shrinker edits)
//   sites.json     the site table, glue (auto-sites), hit ledger and plan (what the comparator reads)
//
// The oracle copies only source files, so the JSON and .txt files are inert to it: the case
// directory runs as it is (clean) with `scripts/runtime-oracle.mjs --cases`.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { GENERATOR_VERSION } from "./generate.mjs";
import { HITS_SELECTOR, haltable, stripInstrumentation } from "./hits.mjs";
import { drivingPlan, moduleFiles, printProgram, programPackages } from "./print.mjs";

const clickSelector = (id) => `[data-act="${id}"]`;

/**
 * The scenario steps for a plan: its edits (`{ selector, text }`: the user types into an input, before any click), clicks in order,
 * then one read per distinct witness selector. An edit is not `haltable` (a halt marker cannot be typed into: a halted run fails there).
 * `hits` appends two reads of the hit readout last (their checkpoints are the counters; equal texts show the page was still).
 * Every click and witness read is `haltable`: after REACTIVITY_HALTED a missing element resolves to the oracle's halt marker
 * (`haltMarker` in oracle.json) instead of holding the run for the scenario timeout. chrome.mjs reads such a checkpoint as absent
 * and the run as cut short (a partial run).
 */
export function scenarioSteps(plan, sites, { hits = false } = {}) {
  const steps = [...(plan.edits ?? []).map((edit) => ({ action: "fill", selector: edit.selector, text: edit.text })),
    ...plan.clicks.map((id) => ({ action: "click", selector: haltable(clickSelector(id)) }))];
  const seen = new Set();
  for (const site of sites) {
    const selector = site.witness?.selector;
    if (!selector || seen.has(selector)) continue;
    seen.add(selector);
    steps.push({ action: "wait-for-selector", selector: haltable(selector) });
  }
  const kept = steps.slice(0, hits ? 58 : 60);
  if (hits) for (let read = 0; read < 2; read += 1) kept.push({ action: "wait-for-selector", selector: HITS_SELECTOR });
  return kept;
}

export function buildCase(program, { id = `fuzz-${program.seed}`, note } = {}) {
  const { text, runText, sites, glue, ledger, lineTable } = printProgram(program);
  const plan = drivingPlan(program);
  const steps = scenarioSteps(plan, sites);
  const runSteps = scenarioSteps(plan, sites, { hits: true });
  // Neutrality, part (a): stripping every counter from the twin must give the clean text back, line for line.
  const neutral = stripInstrumentation(runText) === text;
  // Generated packages: `node_modules/<name>/...` in the case directory. The oracle copies no node_modules of a case, so
  // `oracle.json` names the packages it must install into the case's own node_modules (never the shared install's).
  const packages = programPackages(program);
  const packageNames = Object.keys(packages);
  const packageFiles = Object.fromEntries(packageNames.flatMap((name) => Object.entries(packages[name]).map(([file, text]) => [`node_modules/${name}/${file}`, text])));
  const description = note ?? `differential fuzzer, generator ${GENERATOR_VERSION}, seed ${program.seed}`;
  return {
    id, text, runText, sites, glue, ledger, lineTable, plan, neutral, packages,
    files: {
      ...packageFiles,
      // The project's other modules (`helpers.ts`): the same text in the clean case and the twin (they hold no site).
      ...moduleFiles(program),
      "App.tsx": text,
      "App.run.txt": runText,
      "oracle.json": `${JSON.stringify({ note: description, ...(packageNames.length ? { packages: packageNames } : {}), haltMarker: true, steps }, null, 2)}\n`,
      "oracle.run.json": `${JSON.stringify({ note: description, ...(packageNames.length ? { packages: packageNames } : {}), haltMarker: true, steps: runSteps }, null, 2)}\n`,
      "program.json": `${JSON.stringify(program)}\n`,
      "sites.json": `${JSON.stringify({ id, generator: GENERATOR_VERSION, seed: program.seed, plan, sites, glue, ledger }, null, 2)}\n`,
    },
  };
}

export function writeCase(directory, built) {
  mkdirSync(directory, { recursive: true });
  for (const [name, contents] of Object.entries(built.files)) {
    mkdirSync(dirname(join(directory, name)), { recursive: true });
    writeFileSync(join(directory, name), contents);
  }
  return directory;
}
