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
import { join } from "node:path";
import { GENERATOR_VERSION } from "./generate.mjs";
import { HITS_SELECTOR, stripInstrumentation } from "./hits.mjs";
import { drivingPlan, printProgram } from "./print.mjs";

const clickSelector = (id) => `[data-act="${id}"]`;

/**
 * The scenario steps for a plan: clicks in order, then one read per distinct witness selector.
 * `hits` appends two reads of the hit readout last (their checkpoints are the counters; equal texts show the page was still).
 */
export function scenarioSteps(plan, sites, { hits = false } = {}) {
  const steps = plan.clicks.map((id) => ({ action: "click", selector: clickSelector(id) }));
  const seen = new Set();
  for (const site of sites) {
    const selector = site.witness?.selector;
    if (!selector || seen.has(selector)) continue;
    seen.add(selector);
    steps.push({ action: "wait-for-selector", selector });
  }
  const kept = steps.slice(0, hits ? 58 : 60);
  if (hits) for (let read = 0; read < 2; read += 1) kept.push({ action: "wait-for-selector", selector: HITS_SELECTOR });
  return kept;
}

export function buildCase(program, { id = `fuzz-${program.seed}`, note } = {}) {
  const { text, runText, sites, glue, ledger } = printProgram(program);
  const plan = drivingPlan(program);
  const steps = scenarioSteps(plan, sites);
  const runSteps = scenarioSteps(plan, sites, { hits: true });
  // Neutrality, part (a): stripping every counter from the twin must give the clean text back, line for line.
  const neutral = stripInstrumentation(runText) === text;
  const description = note ?? `differential fuzzer, generator ${GENERATOR_VERSION}, seed ${program.seed}`;
  return {
    id, text, runText, sites, glue, ledger, plan, neutral,
    files: {
      "App.tsx": text,
      "App.run.txt": runText,
      "oracle.json": `${JSON.stringify({ note: description, steps }, null, 2)}\n`,
      "oracle.run.json": `${JSON.stringify({ note: description, steps: runSteps }, null, 2)}\n`,
      "program.json": `${JSON.stringify(program)}\n`,
      "sites.json": `${JSON.stringify({ id, generator: GENERATOR_VERSION, seed: program.seed, plan, sites, glue, ledger }, null, 2)}\n`,
    },
  };
}

export function writeCase(directory, built) {
  mkdirSync(directory, { recursive: true });
  for (const [name, contents] of Object.entries(built.files)) writeFileSync(join(directory, name), contents);
  return directory;
}
