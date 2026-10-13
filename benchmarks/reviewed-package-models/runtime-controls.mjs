// Independent samples execute published package bytes, never analysis twins.
import assert from "node:assert/strict";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { read } from "./catalog.mjs";
const [runPath, outPath] = process.argv.slice(2), run = read(resolve(runPath)), out = resolve(outPath);
assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const snippets = {
  raf: `import assert from "node:assert/strict";
import { OBSERVE, createRoot, createComponent, createEffect, flush } from "solid-js";
import { createRAF } from "@solid-primitives/raf";
globalThis.cancelAnimationFrame = () => {};
const capture = OBSERVE.diagnostics.capture();
createRAF(() => {});
const unowned = capture.events.map(({code}) => code); capture.clear();
let dispose = createRoot(dispose => {
  createComponent(() => { const [running] = createRAF(() => {}); running(); return null; }, {});
  return dispose;
});
const eager = capture.events.map(({code}) => code); dispose(); capture.clear();
dispose = createRoot(dispose => {
  createComponent(() => { const [running] = createRAF(() => {}); createEffect(() => running(), () => {}); return null; }, {});
  return dispose;
});
flush(); const tracked = capture.events.map(({code}) => code); dispose(); capture.stop();
assert.deepEqual(unowned, ["NO_OWNER_CLEANUP"]);
assert.deepEqual(eager, ["STRICT_READ_UNTRACKED"]); assert.deepEqual(tracked, []);
console.log(JSON.stringify({unowned,eager,tracked}));`,
  lifecycle: `import assert from "node:assert/strict";
import { OBSERVE, flush } from "solid-js";
import { createIsMounted } from "@solid-primitives/lifecycle";
const capture = OBSERVE.diagnostics.capture(); const read = createIsMounted(); flush();
const value = read(), diagnostics = capture.stop().map(({code}) => code);
assert.equal(value, true); assert.deepEqual(diagnostics, []);
console.log(JSON.stringify({value,diagnostics}));`,
  media: `import assert from "node:assert/strict";
import { OBSERVE, createRoot, createComponent } from "solid-js";
let added = 0, removed = 0;
globalThis.window = { matchMedia: () => ({ matches: true,
  addEventListener() { added++; }, removeEventListener() { removed++; } }) };
// utils chooses isClient/isDev during module initialization. Browser globals
// must exist before its module evaluates, just as they do in an actual browser.
const { createMediaQuery, createPrefersDark } = await import("@solid-primitives/media");
const capture = OBSERVE.diagnostics.capture();
createMediaQuery("(min-width: 1px)"); createPrefersDark();
const unowned = capture.events.map(({code}) => code), unownedListeners = { added, removed }; capture.clear();
const dispose = createRoot(dispose => { createMediaQuery("(min-width: 1px)"); return dispose; });
dispose(); const owned = capture.events.map(({code}) => code); capture.clear();
const eagerDispose = createRoot(dispose => { createComponent(() => {
  const read = createMediaQuery("(min-width: 1px)"); read(); return null;
}, {}); return dispose; });
const eager = capture.events.map(({code}) => code); eagerDispose(); capture.stop();
assert.deepEqual(unowned, []); assert.deepEqual(unownedListeners, { added: 2, removed: 0 });
assert.deepEqual(owned, []); assert.equal(removed, 2); assert.deepEqual(eager, ["STRICT_READ_UNTRACKED"]);
console.log(JSON.stringify({unowned,unownedListeners,owned,eager,finalListeners:{added,removed}}));`,
  scheduled: `import assert from "node:assert/strict";
import { OBSERVE } from "solid-js";
import { debounce } from "@solid-primitives/scheduled";
const capture = OBSERVE.diagnostics.capture(), run = debounce(() => {}, 1000);
run(); run.clear(); const diagnostics = capture.stop().map(({code}) => code);
assert.deepEqual(diagnostics, []); console.log(JSON.stringify({diagnostics}));`,
};
const results = { authority: false, runtime: "2.0.0-rc.9", host: "browser resolution in Node with mocked RAF cancellation and media queries", results: {} };
for (const [name, code] of Object.entries(snippets)) {
  const row = run.results.find(item => item.package === `@solid-primitives/${name}`);
  assert(row && row.solid["solid-js"] === "2.0.0-rc.9");
  writeFileSync(join(out, `${name}.mjs`), code);
  const response = spawnSync(process.execPath, ["--conditions=browser", "--conditions=development", "--input-type=module"], {
    cwd: row.retainedArtifacts.projectDir, input: code, encoding: "utf8", timeout: 10000,
  });
  results.results[name] = { package: row.package, version: row.version, stdout: response.stdout, stderr: response.stderr, exitCode: response.status };
  writeFileSync(join(out, "results.json"), JSON.stringify(results, null, 2) + "\n");
  assert.equal(response.status, 0, response.stderr);
  results.results[name] = { package: row.package, version: row.version, observation: JSON.parse(response.stdout), stderr: response.stderr };
}
writeFileSync(join(out, "results.json"), JSON.stringify(results, null, 2) + "\n");
console.log(JSON.stringify(results, null, 2));
