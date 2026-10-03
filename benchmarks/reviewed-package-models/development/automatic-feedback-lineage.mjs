// Fresh precision challenges: callbacks whose returned values are discarded
// by their enclosing computation, plus a named callback whose result is used.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
const output = resolve(process.argv[2]); assert(!existsSync(output)); mkdirSync(output);
const base = resolve("rust/target/development-live-cli-v6/async-body-return-serial-adopted-child-object-deferred/original");
const configuration = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8"));
const bodies = [
  { id: "discarded-inline-queue-result", role: "control", expected: "9", body: "void queue.enqueue(async()=>{await Promise.resolve();const value=get();h.done=true;return {value};});return {value:9};" },
  { id: "discarded-inline-reaction-result", role: "control", expected: "9", body: "void Promise.resolve().then(async()=>{const value=get();h.done=true;return {value};});return {value:9};" },
  { id: "discarded-named-queue-result", role: "control", expected: "9", body: "const task=async()=>{await Promise.resolve();const value=get();h.done=true;return {value};};h.values.taskName=task.name;void queue.enqueue(task);return {value:9};" },
  { id: "used-named-queue-result", role: "target", expected: "1", body: "const task=async()=>{await Promise.resolve();const value=get();h.done=true;return {value};};h.values.taskName=task.name;return queue.enqueue(task);" }
  ,{ id: "discarded-method-read-result", role: "control", expected: "9", body: "return queue.enqueue(async()=>{await Promise.resolve();const reader={run(){get();return {value:9};}};h.done=true;return reader.run();});" }
  ,{ id: "used-method-read-result", role: "target", expected: "1", body: "return queue.enqueue(async()=>{await Promise.resolve();const reader={run(){return {value:get()};}};h.done=true;return reader.run();});" }
], results = [];
for (const row of bodies) {
  const root = join(output, row.id); mkdirSync(join(root, "src"), { recursive: true });
  symlinkSync(join(base, "node_modules"), join(root, "node_modules"), "dir");
  for (const name of ["index.html", "tsconfig.json"]) writeFileSync(join(root, name), readFileSync(join(base, name)));
  for (const name of ["setup.ts", "entry.ts"]) writeFileSync(join(root, "src", name), readFileSync(join(base, "src", name)));
  writeFileSync(join(root, "src/main.tsx"), `import {createMemo,createSignal,Loading,flush} from 'solid-js';import {render} from '@solidjs/web';
    import {createTaskQueue} from '@solid-primitives/queue';const h=(globalThis as any).__experiment;h.done=false;
    function App(){const [value,set]=createSignal(1),get=()=>value();const queue=createTaskQueue<{value:number}>();
      h.update=()=>{set(2);flush();};const result=createMemo(()=>{${row.body}});
      h.probePending=()=>!h.done||h.checkPending(()=>result().value);
      return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p><p id='task-name'>{String(h.values.taskName??'none')}</p></Loading>;}
    h.dispose=render(()=><App/>,document.getElementById('root')!);`);
  const scenario = join(root, "scenario.json"); writeFileSync(scenario, JSON.stringify({ schemaVersion: 1, steps: [
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' }, { action: "click", selector: "#update" },
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' }, { action: "wait-for-selector", selector: "#task-name" },
    { action: "wait-for-selector", selector: "#value" }
  ] }));
  const child = spawnSync(process.execPath, [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", join(root, "tsconfig.json"),
    "--scenario", scenario, "--browser", configuration.binaries.chromium, "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")],
    { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  assert([0, 1].includes(child.status), child.error?.message ?? child.stderr);
  const report = JSON.parse(child.stdout); writeFileSync(join(root, "feedback.json"), JSON.stringify(report, null, 2) + "\n");
  assert.equal(report.typingErrorCount, 0); assert.deepEqual(report.execution.pageErrors, []); assert.deepEqual(report.assertions, []);
  assert.equal(report.execution.checkpoints.at(-1).text, row.expected);
  if (row.id.includes("named")) assert.equal(report.execution.checkpoints.at(-2).text, "task");
  results.push({ id: row.id, role: row.role, actual: report.execution.checkpoints.at(-1).text, automaticWarnings: report.automatic.notes.filter(note => note.severity === "warning").length });
  writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false, results }, null, 2)); console.log(JSON.stringify(results.at(-1)));
}
