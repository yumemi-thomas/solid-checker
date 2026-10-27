// Emits a program as an oracle case directory, so Chrome confirmation is just
// `scripts/runtime-oracle.mjs --cases <parent> --only <id>`.
//
//   App.tsx        the program (every exported capitalized function is rendered by the oracle)
//   oracle.json    { note, steps }: the driving plan as scenario steps (click each plan button,
//                  then read each witness selector so the report keeps its text)
//   program.json   the construct tree (what the shrinker edits)
//   sites.json     the site table and plan (what the comparator reads)
//
// The oracle copies only source files, so the JSON files are inert to it.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { GENERATOR_VERSION } from "./generate.mjs";
import { drivingPlan, printProgram } from "./print.mjs";

const clickSelector = (id) => `[data-act="${id}"]`;

/** The scenario steps for a plan: clicks in order, then one read per distinct witness selector. */
export function scenarioSteps(plan, sites) {
  const steps = plan.clicks.map((id) => ({ action: "click", selector: clickSelector(id) }));
  const seen = new Set();
  for (const site of sites) {
    const selector = site.witness?.selector;
    if (!selector || seen.has(selector)) continue;
    seen.add(selector);
    steps.push({ action: "wait-for-selector", selector });
  }
  return steps.slice(0, 60);
}

export function buildCase(program, { id = `fuzz-${program.seed}`, note } = {}) {
  const { text, sites } = printProgram(program);
  const plan = drivingPlan(program);
  const steps = scenarioSteps(plan, sites);
  const description = note ?? `differential fuzzer, generator ${GENERATOR_VERSION}, seed ${program.seed}`;
  return {
    id, text, sites, plan,
    files: {
      "App.tsx": text,
      "oracle.json": `${JSON.stringify({ note: description, steps }, null, 2)}\n`,
      "program.json": `${JSON.stringify(program)}\n`,
      "sites.json": `${JSON.stringify({ id, generator: GENERATOR_VERSION, seed: program.seed, plan, sites }, null, 2)}\n`,
    },
  };
}

export function writeCase(directory, built) {
  mkdirSync(directory, { recursive: true });
  for (const [name, contents] of Object.entries(built.files)) writeFileSync(join(directory, name), contents);
  return directory;
}
