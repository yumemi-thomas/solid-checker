import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

const recipe = new URL("./probe-recipes/web-with-meta.mjs", import.meta.url);

// These isolated input adapters test the recipe's observation, not published
// package behavior or certification. Actual rc.3 artifact certification is a
// separate measurement. Copy the recipe unchanged, retaining its public
// specifier and browser condition selection; no installed package is edited.
function observeRecipe(behavior) {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-with-meta-recipe-"));
  try {
    const packageRoot = join(directory, "node_modules", "@solidjs", "web");
    mkdirSync(packageRoot, { recursive: true });
    writeFileSync(join(packageRoot, "package.json"), JSON.stringify({
      name: "@solidjs/web",
      version: "0.0.0-test-adapter",
      type: "module",
      exports: {
        "./server-functions": {
          browser: { import: "./client.mjs" },
          node: { import: "./server.mjs" }
        }
      }
    }));
    writeFileSync(join(packageRoot, "server.mjs"),
      'throw new Error("the recipe test requires the browser artifact");\n');
    writeFileSync(join(packageRoot, "client.mjs"), `
const behavior = ${JSON.stringify(behavior)};
const metadataKey = Symbol("test-adapter metadata");
export const observations = { names: [], calls: 0, invocations: 0, frozenPatches: 0 };
export function createServerReference(id, name) {
  if (typeof id !== "string" || arguments.length !== 2) {
    throw new Error("unexpected constructor arguments");
  }
  observations.names.push(name ?? null);
  const fn = () => {
    observations.invocations += 1;
    throw new Error("the recipe must never invoke the server reference");
  };
  fn[metadataKey] = name === undefined ? {} : { name };
  return new Proxy(fn, {});
}
export function getServerFunctionMetadata(fn) {
  return typeof fn === "function" ? fn[metadataKey] : undefined;
}
export function withMeta(fn, patch) {
  const metadata = getServerFunctionMetadata(fn);
  if (!metadata || arguments.length !== 2 || typeof patch !== "object") {
    throw new Error("invalid sample");
  }
  observations.calls += 1;
  observations.frozenPatches += Number(Object.isFrozen(patch));
  if (behavior === "throw") throw new Error("adapter refuses every sample");
  if (behavior !== "no-update") Object.assign(metadata, patch);
  return behavior === "wrong-return" ? new Proxy(fn, {}) : fn;
}
`);
    copyFileSync(recipe, join(directory, "recipe.mjs"));
    writeFileSync(join(directory, "run.mjs"), `
import { runProbeSession } from "./recipe.mjs";
import { observations } from "@solidjs/web/server-functions";
const events = [];
globalThis.fetch = () => { throw new Error("network is forbidden in this test"); };
let error = null;
try {
  await runProbeSession(Object.freeze({}), Object.freeze({ emit: event => events.push(event) }));
} catch (caught) {
  error = caught.message;
}
process.stdout.write(JSON.stringify({ events, observations, error }));
`);
    return JSON.parse(execFileSync("node", ["--conditions=browser", join(directory, "run.mjs")], {
      cwd: directory,
      encoding: "utf8",
      timeout: 10_000
    }));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

const callEnter = { marker: "call", kind: "call", phase: "enter" };
const callExit = { marker: "call", kind: "call", phase: "exit" };
const contradiction = { marker: "return-outside-identity", kind: "call", phase: "enter" };

test("withMeta recipe completes four branded-reference samples without invoking them", () => {
  const result = observeRecipe("clean");
  assert.equal(result.error, null);
  assert.deepEqual(result.observations, {
    names: [null, "solid-checker:with-meta-named"],
    calls: 4,
    invocations: 0,
    frozenPatches: 2
  });
  assert.deepEqual(result.events, Array.from({ length: 4 }, () => [callEnter, callExit]).flat());
});

test("withMeta recipe emits the exact call-class veto for a distinct returned callable", () => {
  const result = observeRecipe("wrong-return");
  assert.equal(result.error, null);
  assert.equal(result.observations.calls, 4);
  assert.equal(result.observations.invocations, 0);
  assert.deepEqual(result.events,
    Array.from({ length: 4 }, () => [callEnter, contradiction, callExit]).flat());
});

test("withMeta recipe refuses an identity-only adapter that never updates metadata", () => {
  const result = observeRecipe("no-update");
  assert.equal(result.error, "withMeta probe did not observe the metadata update");
  assert.equal(result.observations.calls, 1);
  assert.deepEqual(result.events, [callEnter, callExit]);
});

test("withMeta recipe propagates a throwing sample instead of claiming completion", () => {
  const result = observeRecipe("throw");
  assert.equal(result.error, "adapter refuses every sample");
  assert.equal(result.observations.calls, 1);
  assert.deepEqual(result.events, [callEnter]);
});
