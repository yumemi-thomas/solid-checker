// Temporary-v2 runtime-probe orchestration. Native Rust authorizes sessions
// and classifies raw worker events; this layer only manages files/processes.

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { arch, platform } from "node:os";
import { resolve } from "node:path";
import process from "node:process";

import { runNative } from "../bin/launcher.mjs";
import { runProbeSessions } from "./contract-probe-driver.mjs";

export const contractProbeHelp = `Usage:
  solid-checker contract probe <PROPOSAL> --request <FILE> [OPTIONS]

The request binds exact artifact modes and claim-addressed recipe modules to
the Rust-issued proposal plan. Every repeat runs in a fresh process. Rust
classifies raw semantic events and treats finite non-observation, timeout,
error, environment mismatch, or inconsistent repeats as local refusal.

Options:
  --proposal-plan <FILE>  Rust proposal plan (default: <PROPOSAL>.proposal.json)
  --request <FILE>        Temporary-v2 mode/recipe request (required)
  --plan <FILE>           Native session plan output (default: <PROPOSAL>.probe-plan.json)
  --runs <FILE>           Raw worker runs output (default: <PROPOSAL>.probe-runs.json)
  --report <FILE>         Rust evaluation output (default: <PROPOSAL>.probe.json)
  --plan-only             Validate and write sessions without executing package code
`;

function usage(message) {
  return new Error(`${message}\n\n${contractProbeHelp}`);
}

export function parseProbeArguments(arguments_) {
  const options = { proposal: "", proposalPlan: "", request: "", plan: "", runs: "", report: "", planOnly: false, help: false };
  const fields = new Map([["--proposal-plan", "proposalPlan"], ["--request", "request"], ["--plan", "plan"], ["--runs", "runs"], ["--report", "report"]]);
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) options.help = true;
    else if (argument === "--plan-only") options.planOnly = true;
    else {
      const separator = argument.indexOf("=");
      const key = separator < 0 ? argument : argument.slice(0, separator);
      const field = fields.get(key);
      if (field) {
        const value = separator < 0 ? arguments_[++index] : argument.slice(separator + 1);
        if (!value) throw usage(`${key} requires a value`);
        options[field] = value;
      } else if (argument.startsWith("-")) throw usage(`unknown contract probe argument ${argument}`);
      else if (options.proposal) throw usage(`unexpected argument ${argument}`);
      else options.proposal = argument;
    }
  }
  if (!options.help && (!options.proposal || !options.request)) {
    throw usage("contract probe requires a proposal and --request");
  }
  return options;
}

function native(args) {
  const child = runNative("solid-checker", args);
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) throw new Error(child.stderr?.trim() || child.stdout?.trim() || `native checker exited ${child.status}`);
}

function producer() {
  const build = createHash("sha256").update(`${process.execPath}\0${process.version}`).digest("hex");
  return { name: "solid-checker-node-probe-driver", version: "2", build: `sha256:${build}`, protocol: "solid-checker-runtime-probe-v2" };
}

export async function probeContract(arguments_) {
  const options = parseProbeArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractProbeHelp);
    return;
  }
  const proposal = resolve(options.proposal);
  const proposalPlan = resolve(options.proposalPlan || `${proposal}.proposal.json`);
  const request = resolve(options.request);
  const planPath = resolve(options.plan || `${proposal}.probe-plan.json`);
  const runsPath = resolve(options.runs || `${proposal}.probe-runs.json`);
  const report = resolve(options.report || `${proposal}.probe.json`);
  const base = ["--runtime-probe-proposal", proposal, "--runtime-probe-proposal-plan", proposalPlan, "--runtime-probe-request", request, "--runtime-probe-plan-output", planPath];
  native(base);
  if (options.planOnly) {
    process.stdout.write(`wrote temporary-v2 runtime probe plan to ${planPath}\n`);
    return { proposal, plan: planPath, schemaVersion: 2, executed: false };
  }
  const plan = JSON.parse(readFileSync(planPath, "utf8"));
  const runs = await runProbeSessions(plan, request);
  writeFileSync(runsPath, `${JSON.stringify({ format: "solid-checker-runtime-probe-runs", schemaVersion: 2, planDigest: plan.planDigest, producer: producer(), runs }, null, 2)}\n`);
  native([...base, "--runtime-probe-runs", runsPath, "--runtime-probe-evaluation-output", report]);
  process.stdout.write(`wrote Rust-classified temporary-v2 runtime probe evaluation to ${report}\n`);
  return { proposal, plan: planPath, runs: runsPath, report, schemaVersion: 2, executed: true, os: platform(), architecture: arch() };
}
