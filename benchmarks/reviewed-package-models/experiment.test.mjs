import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import { authenticateModel, read } from "./catalog.mjs";
import { lower, projectWarning, ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const here = dirname(fileURLToPath(import.meta.url)), repo = resolve(here, "../..");
const catalog = read(join(here, "models.json"));
const retained = read(process.env.REVIEWED_MODEL_RETAINED_RUN ?? join(repo, "rust/target/primitives-checkpoint/run-browser.json"));
const media = catalog.models.find(model => model.package === "@solid-primitives/media");
const project = retained.results.find(row => row.package === media.package).retainedArtifacts.projectDir;
assert(existsSync(project), "Tests require the retained published install; missing inputs must not skip");

test("version and dependency bytes are checked before any behavior is admitted", () => {
  assert.throws(() => authenticateModel({ ...media, version: "0.0.0" }, project), /version mismatch/);
  const changed = structuredClone(media); changed.pins[0].digest = "sha256:wrong";
  assert.throws(() => authenticateModel(changed, project), /input bytes changed/);
});

test("unsupported expression placement remains unknown", () => {
  const dir = mkdtempSync(join(tmpdir(), "solid-reviewed-model-test-"));
  symlinkSync(join(project, "node_modules"), join(dir, "node_modules"), "dir");
  const path = join(dir, "App.tsx");
  writeFileSync(path, `import { createMediaQuery } from "@solid-primitives/media";
export default function App() { return <p>{createMediaQuery("x")()}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions("v2", true), dir).options;
  const program = ts.createProgram([path], options);
  const result = lower(program, program.getSourceFile(path), catalog, "browser");
  assert.equal(result.sites.length, 1); assert.equal(result.sites[0].applied, undefined);
  assert.match(result.unsupported[0].reason, /placement/);
});

test("warning locations map UTF-8 bytes back to the original caller", () => {
  const path = "/original/App.tsx", modeledPath = "/modeled/App.tsx";
  const text = "// 日本語\nconst read = factory();\nconst value = read();";
  const original = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const prefix = "// generated\n", name = text.indexOf("read"), call = text.lastIndexOf("read()");
  const generated = prefix + text;
  const location = position => ({ path: modeledPath, startByte: Buffer.byteLength(generated.slice(0, prefix.length + position)) });
  const lowered = { text: generated, segments: [
    { start: 0, end: prefix.length, originalStart: 0, copied: false },
    { start: prefix.length, end: generated.length, originalStart: 0, copied: true },
  ], sites: [{ package: "pkg", export: "factory", modelVersion: "1", applied: true, behavior: { returns: "accessor" },
    start: text.indexOf("factory()"), end: text.indexOf("factory()") + 9, bindings: [{ start: name, end: name + 4 }] }] };
  const warning = projectWarning({ kind: "violation", id: "SC1001", rule: "strict-read-untracked", message: "read", primaryLocation: location(call),
    relatedLocations: [location(name)] }, lowered, original, modeledPath);
  assert.equal(warning.location.startByte, Buffer.byteLength(text.slice(0, call)));
  assert.equal(warning.location.line, 3); assert.equal(warning.location.path, path);
  assert.equal(warning.certification, false); assert.equal(warning.basis, "reviewed-source-assumption");
});

test("an accessor surrogate cannot add a package ownership requirement", () => {
  const path = "/original/App.tsx", modeledPath = "/modeled/App.tsx";
  const text = "const read = factory();", start = text.indexOf("factory()");
  const original = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true);
  const lowered = { text, segments: [{ start: 0, end: text.length, originalStart: 0, copied: true }],
    sites: [{ package: "pkg", export: "factory", start, applied: true, behavior: { returns: "accessor" }, bindings: [] }] };
  for (const rule of ["missing-owner", "leaf-owner-forbidden-call", "reactive-write-in-owned-scope"])
    assert.equal(projectWarning({ kind: "violation", rule, primaryLocation: { path: modeledPath, startByte: start } }, lowered, original, modeledPath), null);
});

test("a copied unmodeled callback does not inherit a lifetime premise", () => {
  const path = "/original/App.tsx", modeledPath = "/modeled/App.tsx";
  const text = "const read = () => 1; factory(() => read());";
  const original = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const start = text.indexOf("factory"), callbackRead = text.lastIndexOf("read()");
  const warning = projectWarning({ kind: "violation", id: "SC1001", rule: "strict-read-untracked", message: "read",
    primaryLocation: { path: modeledPath, startByte: callbackRead }, relatedLocations: [{ path: modeledPath, startByte: text.indexOf("read") }] },
  { text, segments: [{ start: 0, end: text.length, copied: true, originalStart: 0 }],
    sites: [{ package: "pkg", export: "factory", modelVersion: "1", applied: true, behavior: { owner: "cleanup" }, start, end: text.length, bindings: [] }] },
  original, modeledPath);
  assert.equal(warning, null);
});
test('a zero-argument owner premise survives an opaque published tuple result', () => {
  const date = catalog.models.find(model => model.package === '@solid-primitives/date');
  const installed = retained.results.find(row => row.package === date.package).retainedArtifacts.projectDir;
  const dir = mkdtempSync(join(tmpdir(), 'solid-reviewed-owner-tuple-')); symlinkSync(join(installed, 'node_modules'), join(dir, 'node_modules'), 'dir');
  const path = join(dir, 'App.tsx'), code = `import { createDateNow } from '@solid-primitives/date'; export const tuple = createDateNow();`;
  writeFileSync(path, code); const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), dir).options;
  const program = ts.createProgram([path], options); assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const result = lower(program, program.getSourceFile(path), catalog, 'browser'); assert.deepEqual(result.sites[0].appliedPremises, ['owner']);
  assert(result.text.includes('createDateNow()')); assert(result.unsupported.some(row => row.reason.includes('tuple return remains opaque')));
  writeFileSync(path, result.text); const modeled = ts.createProgram([path], options); assert.equal(ts.getPreEmitDiagnostics(modeled).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const source = program.getSourceFile(path), site = result.sites[0];
  const position = result.segments.find(s => !s.copied && s.originalStart === site.start).start;
  const location = { path, startByte: Buffer.byteLength(result.text.slice(0, position)) };
  const finding = { kind: 'violation', rule: 'missing-owner', primaryLocation: location };
  assert(projectWarning(finding, result, source, path));
  assert.equal(projectWarning({ ...finding, rule: 'strict-read-untracked' }, result, source, path), null);
  const withheld = structuredClone(catalog); delete withheld.models.find(model => model.package === date.package).exports.createDateNow.browser.owner;
  const withoutOwner = lower(program, source, withheld, 'browser'); assert.equal(withoutOwner.sites[0].applied, undefined);
});
