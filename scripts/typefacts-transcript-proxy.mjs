#!/usr/bin/env node

// Transparent test-only proxy for recording the exact framed process protocol.
// The target and two output paths are explicit environment variables so the
// captured bytes never become an implicit production dependency.

import { appendFileSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";

const target = process.env.TYPEFACTS_TRANSCRIPT_TARGET;
const requests = process.env.TYPEFACTS_TRANSCRIPT_REQUESTS;
const responses = process.env.TYPEFACTS_TRANSCRIPT_RESPONSES;
if (!target || !requests || !responses) {
  console.error("typefacts transcript proxy requires TARGET, REQUESTS, and RESPONSES paths");
  process.exit(2);
}

writeFileSync(requests, Buffer.alloc(0));
writeFileSync(responses, Buffer.alloc(0));

// Keep transitions inline for reproducible transcripts. The client may create
// an arena and pass its random path, but it accepts an inline response when the
// producer does not opt into that transport optimization.
const targetArgs = process.argv.slice(2).filter(arg => !arg.startsWith("-transition-arena="));
const child = spawn(target, targetArgs, {
  stdio: ["pipe", "pipe", "inherit"],
  env: process.env,
});

process.stdin.on("data", chunk => {
  appendFileSync(requests, chunk);
  child.stdin.write(chunk);
});
process.stdin.on("end", () => child.stdin.end());
child.stdout.on("data", chunk => {
  appendFileSync(responses, chunk);
  process.stdout.write(chunk);
});
child.on("error", error => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on("close", (code, signal) => {
  process.exitCode = signal ? 1 : (code ?? 1);
});
