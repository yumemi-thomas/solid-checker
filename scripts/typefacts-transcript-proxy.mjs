#!/usr/bin/env node

// Transparent test-only proxy for recording the exact framed process protocol.
// The target and two output paths are explicit environment variables so the
// captured bytes never become an implicit production dependency.

import { appendFileSync, copyFileSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { join } from "node:path";

// Two modes. With TARGET/REQUESTS/RESPONSES the proxy records one session to
// those exact paths (the original single-session form). With
// TYPEFACTS_TRANSCRIPT_DIR it gives every invocation its own numbered
// subdirectory holding requests.bin, responses.bin, argv.json and meta.json,
// so a gate that spawns hundreds of producers (coverage, ownership-gate,
// corpus sweeps) records all of them in one run. TYPEFACTS_TRANSCRIPT_LABEL
// names the recording in meta.json. See scripts/typefacts-differential.mjs.
const target = process.env.TYPEFACTS_TRANSCRIPT_TARGET;
const directory = process.env.TYPEFACTS_TRANSCRIPT_DIR;
let requests = process.env.TYPEFACTS_TRANSCRIPT_REQUESTS;
let responses = process.env.TYPEFACTS_TRANSCRIPT_RESPONSES;
if (!target || (!directory && (!requests || !responses))) {
  console.error(
    "typefacts transcript proxy requires TARGET and either DIR or both REQUESTS and RESPONSES"
  );
  process.exit(2);
}

// Keep transitions inline for reproducible transcripts. The client may create
// an arena and pass its random path, but it accepts an inline response when the
// producer does not opt into that transport optimization.
const targetArgs = process.argv.slice(2).filter(arg => !arg.startsWith("-transition-arena="));
// Sources travel through a temporary arena file the client deletes after
// reading, so a recording could not capture their bytes. Directory recordings
// ask for them inline.
if (directory && !targetArgs.includes("-inline-sources")) targetArgs.unshift("-inline-sources");

let sessionDirectory;
if (directory) {
  mkdirSync(directory, { recursive: true });
  // mkdir without `recursive` is the atomic claim on a sequence number, so
  // concurrent proxies never share a session directory.
  let sequence = readdirSync(directory).length;
  for (;;) {
    sessionDirectory = join(directory, String(sequence).padStart(6, "0"));
    try {
      mkdirSync(sessionDirectory);
      break;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      sequence += 1;
    }
  }
  requests = join(sessionDirectory, "requests.bin");
  responses = join(sessionDirectory, "responses.bin");
  writeFileSync(join(sessionDirectory, "argv.json"), JSON.stringify([target, ...targetArgs]));
  // The checker passes its synthesized project configuration as a temporary
  // file it deletes when the run ends; keep the bytes so the session replays.
  // scripts/typefacts-differential.mjs substitutes this copy on replay.
  const configIndex = targetArgs.findIndex(arg => arg === "-project-config" || arg.startsWith("-project-config="));
  if (configIndex !== -1) {
    const inline = targetArgs[configIndex].startsWith("-project-config=");
    const original = inline ? targetArgs[configIndex].slice("-project-config=".length) : targetArgs[configIndex + 1];
    try {
      copyFileSync(original, join(sessionDirectory, "project-config.bin"));
    } catch (error) {
      console.error(`transcript proxy: cannot keep ${original}: ${error.message}`);
    }
  }
  writeFileSync(
    join(sessionDirectory, "meta.json"),
    JSON.stringify({
      label: process.env.TYPEFACTS_TRANSCRIPT_LABEL ?? "",
      cwd: process.cwd(),
      target,
      recordedAt: new Date().toISOString()
    })
  );
}

writeFileSync(requests, Buffer.alloc(0));
writeFileSync(responses, Buffer.alloc(0));

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
