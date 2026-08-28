// Temporary-v2 proof verification orchestration. The Rust proof checker is
// the only component allowed to close claims or issue receipt bytes.

import { resolve } from "node:path";
import process from "node:process";

import { runNative } from "../bin/launcher.mjs";

export const contractVerifyHelp = `Usage:
  solid-checker contract verify <PROPOSAL> --plan <FILE> --proof <FILE>
    --artifact-case <ID> [--output <FILE>] [--receipt <FILE>]

Replays every required proof family for the selected exact artifact case. On
success Rust writes a finalized temporary-v2 contract and a receipt bound to
its wire, semantic, artifact, closure, proof, and closed-claim roots. Runtime
probe observations can falsify proposed closure but can never replace proof.
`;

function usage(message) {
  return new Error(`${message}\n\n${contractVerifyHelp}`);
}

export function parseVerifyArguments(arguments_) {
  const options = {
    proposal: "",
    plan: "",
    proof: "",
    artifactCase: "",
    output: "",
    receipt: "",
    help: false
  };
  const fields = new Map([
    ["--plan", "plan"],
    ["--proof", "proof"],
    ["--artifact-case", "artifactCase"],
    ["--output", "output"],
    ["--receipt", "receipt"]
  ]);
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) {
      options.help = true;
      continue;
    }
    const separator = argument.indexOf("=");
    const key = separator < 0 ? argument : argument.slice(0, separator);
    const field = fields.get(key);
    if (field) {
      const value = separator < 0 ? arguments_[++index] : argument.slice(separator + 1);
      if (!value) throw usage(`${key} requires a value`);
      options[field] = value;
    } else if (argument.startsWith("-")) {
      throw usage(`unknown contract verify argument ${argument}`);
    } else if (options.proposal) {
      throw usage(`unexpected argument ${argument}`);
    } else {
      options.proposal = argument;
    }
  }
  if (options.help) return options;
  if (!options.proposal) throw usage("contract verify requires a proposal path");
  for (const [flag, field] of [
    ["--plan", "plan"],
    ["--proof", "proof"],
    ["--artifact-case", "artifactCase"]
  ]) {
    if (!options[field]) throw usage(`${flag} is required`);
  }
  return options;
}

export async function verifyContract(arguments_) {
  const options = parseVerifyArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractVerifyHelp);
    return;
  }
  const proposal = resolve(options.proposal);
  const output = resolve(options.output || `${proposal}.accepted.json`);
  const receipt = resolve(options.receipt || `${proposal}.receipt.json`);
  const child = runNative("solid-checker", [
    "--verify-proposal",
    proposal,
    "--verify-plan",
    resolve(options.plan),
    "--verify-proof",
    resolve(options.proof),
    "--verify-artifact-case",
    options.artifactCase,
    "--accepted-output",
    output,
    "--receipt-output",
    receipt
  ]);
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new Error(child.stderr?.trim() || child.stdout?.trim() || `native checker exited ${child.status}`);
  }
  process.stdout.write(`accepted temporary-v2 contract at ${output}; receipt ${receipt}\n`);
  return { proposal, output, receipt, schemaVersion: 2 };
}
