// One isolated runtime-probe-v2 session for a stable-v1 proposal. Recipe modules emit raw
// semantic events; Rust later validates and classifies the complete run.

import { createHash, randomUUID } from "node:crypto";
import { arch, platform } from "node:os";
import { pathToFileURL } from "node:url";

import { createRuntimeProbeHarness } from "./contract-probe-harness.mjs";

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

let input = "";
for await (const chunk of process.stdin) input += chunk;
const session = JSON.parse(input);
const isolation = {
  process: `${process.pid}:${randomUUID()}`,
  realm: randomUUID(),
  moduleInstance: randomUUID()
};
const environment = {
  ...session.mode.environment,
  runtime: {
    name: "node",
    version: process.version,
    build: digest(`${process.execPath}\0${process.version}`),
    protocol: "solid-checker-runtime-probe-v2"
  },
  os: platform(),
  architecture: arch()
};
const harness = createRuntimeProbeHarness(session);
let outcome;
try {
  const module = await import(`${pathToFileURL(process.env.SOLID_CHECKER_PROBE_RECIPE).href}?${isolation.moduleInstance}`);
  if (typeof module.runProbeSession !== "function") {
    outcome = { kind: "refused", reason: "recipe module exports no runProbeSession function" };
  } else {
    const controls = (await module.runProbeSession(session, harness)) ?? {};
    await harness.drain(controls);
    outcome = { kind: "completed", events: harness.events() };
  }
} catch (error) {
  outcome = {
    kind: "error",
    details: digest(error instanceof Error ? error.stack ?? error.message : String(error))
  };
}
process.stdout.write(
  JSON.stringify({
    session: session.id,
    environment,
    isolation,
    drainedMicrotasks: harness.drainedMicrotasks(),
    drainedMacrotasks: harness.drainedMacrotasks(),
    outcome
  })
);
