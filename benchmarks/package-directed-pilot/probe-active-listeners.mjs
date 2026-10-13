// Samples of published event behavior under Node with simulated browser globals.
// These falsify inert behavior; they supply no certification or browser authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const read = path => JSON.parse(readFileSync(path));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const run = read(resolve(process.argv[2])), evidenceFile = resolve(process.argv[3]);
const evidence = read(evidenceFile), out = resolve(process.argv[4]);
assert.equal(evidence.authoredProposals, 0);
assert(!existsSync(out), "Choose a fresh output directory; preserve earlier observations");
assert(process.env.SOLID_CHECKER_PROBE_NODE && existsSync(process.env.SOLID_CHECKER_PROBE_NODE));
mkdirSync(out, { recursive: true });
const specs = [
  { name: "connectivity", export: "makeConnectivityListener", target: "window", event: "online", ownerDisposes: true },
  { name: "media", export: "makeMediaQueryListener", target: "mql", event: "change", ownerDisposes: true },
  { name: "orientation", export: "makeOrientation", target: "orientation", event: "change", ownerDisposes: false }
];
const document = { authority: false, kind: "active-listener-observations", environment: "Node with simulated browser globals",
  evidenceSha256: `sha256:${hash(readFileSync(evidenceFile))}`, results: [] };
for (const spec of specs) {
  const row = run.results.find(item => item.package === `@solid-primitives/${spec.name}`);
  const packageRoot = join(row.retainedArtifacts.projectDir, "node_modules", row.package);
  for (const host of ["node", "browser"]) {
    const observed = evidence.results.find(item => item.package === row.package && item.host === host);
    assert.equal(observed.version, row.version); assert(!observed.refused);
    const catalog = join(resolve(evidenceFile, ".."), `${spec.name}-${host}`, "accepted");
    const pointer = read(join(catalog, "accepted-contracts.json"));
    const accepted = read(join(catalog, pointer.contracts[0].document));
    assert.equal(accepted.package.integrity, observed.integrity);
    const artifact = accepted.entrypoints["."].cases[0].artifact;
    const module = join(packageRoot, artifact.path);
    assert.equal(hash(readFileSync(module)), artifact.sha256);
    const solidRoot = join(row.retainedArtifacts.projectDir, "node_modules", "solid-js");
    const solidManifest = read(join(solidRoot, "package.json"));
    assert.equal(solidManifest.version, "2.0.0-rc.9");
    const ownerRuntime = join(solidRoot, solidManifest.exports["."][host].default);
    const invocation = spec.name === "media" ? `primitive("(min-width: 1px)", callback)` : "primitive(callback)";
    const source = `import assert from "node:assert/strict";
class Target extends EventTarget {
  active = new Map();
  addEventListener(type, callback, options) {
    const callbacks = this.active.get(type) ?? new Set(); callbacks.add(callback); this.active.set(type, callbacks);
    super.addEventListener(type, callback, options);
  }
  removeEventListener(type, callback, options) {
    this.active.get(type)?.delete(callback); super.removeEventListener(type, callback, options);
  }
}
const window = new Target(), mql = new Target(), orientation = new Target();
window.matchMedia = () => mql; orientation.angle = 90; orientation.type = "landscape-primary";
const document = new Target(); document.visibilityState = "visible";
Object.defineProperties(globalThis, { window: {value:window,configurable:true}, document:{value:document,configurable:true},
  screen:{value:{orientation},configurable:true}, navigator:{value:{onLine:true},configurable:true} });
const { ${spec.export}: primitive } = await import(${JSON.stringify(pathToFileURL(module).href)});
const { createRoot } = await import(${JSON.stringify(pathToFileURL(ownerRuntime).href)});
const target = ${spec.target};
const result = [];
for (const owned of [false, true]) {
  let calls = 0; const callback = () => {calls++}; let clear, dispose;
  if (owned) createRoot(disposer => {dispose = disposer; clear = ${invocation};});
  else clear = ${invocation};
  const registered = [...target.active.values()].reduce((sum, callbacks) => sum + callbacks.size, 0);
  target.dispatchEvent(new Event(${JSON.stringify(spec.event)}));
  assert.equal(calls, ${host === "browser" ? 1 : 0});
  const beforeDispose = calls;
  if (owned) dispose();
  target.dispatchEvent(new Event(${JSON.stringify(spec.event)}));
  const afterDispose = calls;
  assert.equal(afterDispose - beforeDispose, ${host === "browser" ? `(owned && ${spec.ownerDisposes} ? 0 : 1)` : "0"});
  clear(); target.dispatchEvent(new Event(${JSON.stringify(spec.event)}));
  assert.equal(calls, afterDispose);
  assert.equal([...target.active.values()].reduce((sum, callbacks) => sum + callbacks.size, 0), 0);
  result.push({owned, registered, beforeDispose, afterDispose, afterClear:calls});
}
console.log(JSON.stringify(result));`;
    writeFileSync(join(out, `${spec.name}-${host}.mjs`), source + "\n");
    const process_ = Bun.spawn([process.env.SOLID_CHECKER_PROBE_NODE, `--conditions=${host}`, "--input-type=module", "--eval", source],
      { stdout: "pipe", stderr: "pipe" });
    const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
    document.results.push({ package: row.package, version: row.version, export: spec.export, host,
      artifactSha256: `sha256:${artifact.sha256}`, ownerRuntimeSha256: `sha256:${hash(readFileSync(ownerRuntime))}`,
      status, stdout, stderr, observation: status === 0 ? JSON.parse(stdout) : null });
    writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2) + "\n");
    assert.equal(status, 0, stderr);
  }
}
console.log(JSON.stringify(document));
