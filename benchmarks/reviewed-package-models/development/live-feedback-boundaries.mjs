// Executable boundary cases against installed published types and runtime.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases from "./cases.mjs";
const [outputArg, referenceArg, mode] = process.argv.slice(2), output = resolve(outputArg), reference = resolve(referenceArg);
assert(!existsSync(output)); mkdirSync(output);
const configuration = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8"));
const retained = cases.find(row => row.id === "async-body-return-serial-adopted-child-object-deferred");
const results = [];
function run(id, project, scenario, comparison) {
  const args = [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", project,
    "--scenario", scenario, "--browser", configuration.binaries.chromium, "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")];
  if (comparison) args.push("--compare-project", comparison);
  const child = spawnSync(process.execPath, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  assert([0, 1].includes(child.status), child.error?.message ?? child.stderr);
  const report = JSON.parse(child.stdout); writeFileSync(join(output, id + ".json"), JSON.stringify(report, null, 2) + "\n");
  return report;
}
const original = join(reference, retained.id, "original"), comparison = join(reference, retained.id, "comparison"), scenario = join(reference, retained.id, "scenario.json");
if (mode !== "exception-only") {
const single = run("without-comparison", join(original, "tsconfig.json"), scenario);
assert.equal(single.assertionFailures.length, 1); assert.deepEqual(single.guidance.notes, []); assert.equal(single.guidance.open.length, 1);
results.push({ id: "without-comparison", assertionFailures: 1, guidance: 0, open: 1 });

const typed = join(output, "typed"); mkdirSync(join(typed, "src"), { recursive: true });
symlinkSync(join(retained.install, "node_modules"), join(typed, "node_modules"), "dir");
for (const name of ["main.tsx", "consumer.ts", "setup.ts", "entry.ts"]) writeFileSync(join(typed, "src", name), readFileSync(join(original, "src", name)));
for (const name of ["index.html", "tsconfig.json"]) writeFileSync(join(typed, name), readFileSync(join(original, name)));
writeFileSync(join(typed, "src/type-error.ts"), `import {createTaskQueue} from '@solid-primitives/queue';
  export function invalidPublishedArgument(){const queue=createTaskQueue<number>();queue.enqueue('not a callback');}`);
const exclusion = run("published-typing-exclusion", join(typed, "tsconfig.json"), scenario, join(comparison, "tsconfig.json"));
assert(exclusion.typingErrorCount > 0); assert.deepEqual(exclusion.assertions, []); assert.deepEqual(exclusion.observations, []); assert.deepEqual(exclusion.guidance.notes, []);
results.push({ id: "published-typing-exclusion", typingErrors: exclusion.typingErrorCount, assertions: 0, observations: 0, guidance: 0 });
}

const pending = join(output, "pending"); mkdirSync(join(pending, "src"), { recursive: true });
symlinkSync(join(retained.install, "node_modules"), join(pending, "node_modules"), "dir");
writeFileSync(join(pending, "tsconfig.json"), readFileSync(join(original, "tsconfig.json")));
writeFileSync(join(pending, "index.html"), '<div id="root"></div><script type="module" src="/src/main.tsx"></script>');
writeFileSync(join(pending, "src/main.tsx"), `import {createMemo,createSignal,createComponent,Loading,flush,isPending,untrack} from 'solid-js';
  import {render} from '@solidjs/web';
  function App(){const [source,set]=createSignal(1);
    const value=createMemo(async()=>{const captured=source();await new Promise<void>(done=>setTimeout(done,20));return captured;},{loadingValue:0});
    const button=document.createElement('button');button.id='update';button.textContent='Update';
    button.onclick=()=>{document.body.dataset.ready='false';set(2);flush();createComponent(()=>value(),{});};document.body.append(button);
    function ready(){if(!isPending(()=>untrack(value)))document.body.dataset.ready='true';requestAnimationFrame(ready);}ready();
    return <Loading fallback={<p>waiting</p>}><p id='value'>{String(value())}</p></Loading>;}
  render(()=><App/>,document.getElementById('root')!);`);
const pendingScenario = join(output, "pending-scenario.json");
writeFileSync(pendingScenario, JSON.stringify({ schemaVersion: 1, steps: [
  { action: "wait-for-text", selector: "#value", text: "1" },
  { action: "wait-for-selector", selector: 'body[data-ready="true"]' },
  { action: "click", selector: "#update" },
  { action: "wait-for-text", selector: "#value", text: "2" },
  { action: "assert-text", id: "settled", selector: "#value", text: "2" }
] }));
const exception = run("native-pending-exception", join(pending, "tsconfig.json"), pendingScenario);
assert.equal(exception.typingErrorCount, 0);
assert(exception.execution.pageErrors.some(error => error.message.includes("PENDING_ASYNC_UNTRACKED_READ")));
assert(exception.observations.some(row => row.kind === "runtime-exception" && row.severity === "error"));
assert.equal(exception.assertionFailures.length, 0);
results.push({ id: "native-pending-exception", typingErrors: 0, runtimeErrors: exception.observations.filter(row => row.kind === "runtime-exception").length, assertionFailures: 0 });
writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false, results }, null, 2) + "\n");
console.log(JSON.stringify(results));
