// Runtime falsifier of the inert dispatcher claim, never proof authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { checkCompletion } from "./check-completion.mjs";

const root = resolve(process.argv[2]);
const observations = JSON.parse(readFileSync(join(root, "results.json")));
checkCompletion(observations);
const node = process.env.SOLID_CHECKER_PROBE_NODE;
assert(node && existsSync(node), "Use the pinned probe Node binary");
const output = join(root, "dispatcher-runtime.json");
assert(!existsSync(output), "Preserve previous runtime evidence");
const results = [];
for (const host of ["node", "browser"]) {
  const trial = observations.results.find(item => item.package === "@solid-primitives/event-dispatcher" && item.host === host);
  const sourcePath = join(trial.packageRoot, trial.artifact.path);
  const sourceSha256 = createHash("sha256").update(readFileSync(sourcePath)).digest("hex");
  assert.equal(sourceSha256, trial.artifact.sha256, "Published source changed since certification");
  const source = `import { createEventDispatcher } from ${JSON.stringify(pathToFileURL(sourcePath).href)};
let getterCalls = 0, handlerCalls = 0;
const props = { get onPing() { getterCalls++; return event => { handlerCalls++; event.preventDefault(); }; } };
const dispatch = createEventDispatcher(props);
const ordinary = dispatch("ping", 1);
const cancelable = dispatch("ping", 2, { cancelable: true });
console.log(JSON.stringify({ getterCalls, handlerCalls, ordinary, cancelable }));`;
  const child = Bun.spawn([node, `--conditions=${host}`, "--input-type=module", "--eval", source],
    { stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([
    child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()
  ]);
  assert.equal(status, 0, stderr);
  assert.equal(stderr, "");
  const actual = JSON.parse(stdout);
  assert.deepEqual(actual, host === "node"
    ? { getterCalls: 0, handlerCalls: 0, ordinary: true, cancelable: true }
    : { getterCalls: 2, handlerCalls: 2, ordinary: true, cancelable: false });
  results.push({ host, package: trial.package, version: trial.version, sourceSha256, observed: actual });
}
writeFileSync(output, JSON.stringify({ authority: false, results }, null, 2) + "\n");
console.log(JSON.stringify({ runtimeControls: results.length, results }));
