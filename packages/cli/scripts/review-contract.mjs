// Stable-v1 review orchestration. Rust expands and inspects normalized
// semantics; JavaScript owns only CLI parsing and process lifecycle.

import { resolve } from "node:path";
import process from "node:process";

import { runNative } from "../bin/launcher.mjs";

export const contractReviewHelp = `Usage:
  solid-checker contract review <PROPOSAL> [--output <FILE>]

Writes a deterministic review document for an unaccepted stable-v1
proposal. The review lists exact artifact cases, exports, and recursively open
semantic claims. It does not edit the proposal, close a claim, or issue an
acceptance receipt.
`;

function usage(message) {
  return new Error(`${message}\n\n${contractReviewHelp}`);
}

export function parseReviewArguments(arguments_) {
  const options = { proposal: "", output: "", help: false };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) {
      options.help = true;
      continue;
    }
    const separator = argument.indexOf("=");
    const key = separator < 0 ? argument : argument.slice(0, separator);
    if (key === "--output") {
      const value = separator < 0 ? arguments_[++index] : argument.slice(separator + 1);
      if (!value) throw usage("--output requires a value");
      options.output = value;
    } else if (argument.startsWith("-")) {
      throw usage(`unknown contract review argument ${argument}`);
    } else if (options.proposal) {
      throw usage(`unexpected argument ${argument}`);
    } else {
      options.proposal = argument;
    }
  }
  if (!options.help && !options.proposal) throw usage("contract review requires a proposal path");
  return options;
}

export async function reviewContract(arguments_) {
  const options = parseReviewArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractReviewHelp);
    return;
  }
  const proposal = resolve(options.proposal);
  const output = resolve(options.output || `${proposal}.review.json`);
  const child = runNative("solid-checker", [
    "--review-contract",
    proposal,
    "--review-output",
    output
  ]);
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new Error(child.stderr?.trim() || child.stdout?.trim() || `native checker exited ${child.status}`);
  }
  process.stdout.write(`wrote stable-v1 contract review to ${output}\n`);
  return { proposal, output, schemaVersion: 2 };
}
