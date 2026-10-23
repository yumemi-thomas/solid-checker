#!/usr/bin/env node
// Cold-start timing of Type Facts producers on one recorded session: each run
// is a fresh process; the clock starts at spawn. Reported per producer (median
// of the runs, milliseconds):
//   handshake  spawn to the startup frame
//   open       spawn to the answer to the session's `open` request (the project
//              is opened before the first request is read, so this includes it)
//   analyze    from that answer to the answer to the session's first `analyze`
//   total      spawn to the last answer of the session
//
//   node scripts/typefacts-reactor-bench.mjs SESSION_DIR [--runs N] NAME=COMMAND...
//
// SESSION_DIR is one session of a differential recording (requests.bin and
// argv.json). A COMMAND is a producer executable; it gets the recorded
// arguments after argv[0]. For a wasm reactor use scripts/typefacts-reactor-stdio.mjs
// with TYPEFACTS_REACTOR_WASM set: NAME=TYPEFACTS_REACTOR_WASM=/path.wasm:scripts/typefacts-reactor-stdio.mjs
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { decodeFrame } from "./lib/typefacts-reactor.mjs";

const args = process.argv.slice(2);
const runsAt = args.indexOf("--runs");
const runs = runsAt === -1 ? 5 : Number(args.splice(runsAt, 2)[1]);
const [session, ...producers] = args;
if (!session || producers.length === 0) {
  console.error("usage: typefacts-reactor-bench.mjs SESSION_DIR [--runs N] NAME=[ENV=VALUE:]COMMAND...");
  process.exit(2);
}

const argv = JSON.parse(readFileSync(join(session, "argv.json"), "utf8")).slice(1);
const requests = [];
for (let bytes = readFileSync(join(session, "requests.bin")); bytes.length;) {
  const size = 4 + bytes.readUInt32LE(0);
  requests.push(bytes.subarray(0, size));
  bytes = bytes.subarray(size);
}
const operations = requests.map(frame => decodeFrame(frame).operation);
const first = name => operations.indexOf(name);
if (first("open") === -1 || first("analyze") === -1) throw new Error("the session has no open and analyze request");

function once(command, env) {
  return new Promise((done, fail) => {
    const started = performance.now();
    const child = spawn(command, argv, { stdio: ["pipe", "pipe", "inherit"], env: { ...process.env, ...env } });
    const marks = [];
    let buffer = Buffer.alloc(0);
    let answers = 0;
    child.on("error", fail);
    child.stdout.on("data", chunk => {
      buffer = Buffer.concat([buffer, chunk]);
      while (buffer.length >= 4 && buffer.length >= 4 + buffer.readUInt32LE(0)) {
        buffer = buffer.subarray(4 + buffer.readUInt32LE(0));
        marks.push(performance.now() - started);
        if (marks.length === 1) {
          child.stdin.write(requests[0]);
        } else if (++answers < requests.length) {
          child.stdin.write(requests[answers]);
        } else {
          child.stdin.end();
        }
      }
    });
    child.on("close", status => {
      if (status !== 0 || marks.length !== requests.length + 1) return fail(new Error(`${command} exited ${status} after ${marks.length} frames`));
      const open = marks[first("open") + 1];
      done({ handshake: marks[0], open, analyze: marks[first("analyze") + 1] - marks[first("analyze")], total: marks.at(-1) });
    });
  });
}

const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
console.log(`session ${session}: ${operations.join(" ")} (${runs} runs)`);
for (const spec of producers) {
  const [name, rest] = [spec.slice(0, spec.indexOf("=")), spec.slice(spec.indexOf("=") + 1)];
  const env = {};
  let command = rest;
  while (/^[A-Z_]+=[^:]*:/.test(command)) {
    const separator = command.indexOf(":");
    const [key, value] = [command.slice(0, command.indexOf("=")), command.slice(command.indexOf("=") + 1, separator)];
    env[key] = resolve(value);
    command = command.slice(separator + 1);
  }
  const samples = [];
  for (let run = 0; run < runs; run++) samples.push(await once(resolve(command), env));
  const row = key => median(samples.map(sample => sample[key])).toFixed(0).padStart(6);
  console.log(`${name.padEnd(12)} handshake${row("handshake")}  open${row("open")}  analyze${row("analyze")}  total${row("total")}`);
}
