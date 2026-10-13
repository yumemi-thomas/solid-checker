import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { performance } from 'node:perf_hooks';
import { hash, read } from './catalog.mjs';
import { ClassFootprints, classSnapshotFlows } from './class-footprints.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [outputArgument, ...browserArguments] = process.argv.slice(2), output = resolve(outputArgument);
assert(!existsSync(output));
const inputNames = ['class-footprints.mjs', 'class-footprint-cases.mjs', 'class-footprint-study.mjs', 'source-extractor.mjs', 'lower.mjs'];
const frozen = inputNames.map(name => ({ name, sha256: hash(readFileSync(new URL(name, import.meta.url))) }));
const started = performance.now(), results = [];
for (const argument of browserArguments) {
  const path = resolve(argument), browser = read(path); assert(browser.finishedAt);
  for (const row of browser.results) {
    const root = join(dirname(path), row.id), main = join(root, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
    assert.equal(hash(readFileSync(main)), row.sourceSha256);
    const result = { id: row.id, provenance: row.provenance, refused: null, browserPath: path,
      browserSha256: hash(readFileSync(path)), behavior: row.values?.behavior ?? null, candidates: [], openUses: [], models: [] }; results.push(result);
    if (row.publishedTypingErrors.length) { result.refused = 'published typings rejected consumer'; continue; }
    if (row.runtime.some(runtime => runtime.version !== '2.0.0-rc.9')) { result.refused = 'runtime outside rc.9'; continue; }
    const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), root).options;
    const ambient = join(root, 'src/vite-env.d.ts');
    const program = ts.createProgram([main, ...(existsSync(ambient) ? [ambient] : [])], options);
    const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
    if (errors.length) { result.refused = 'fresh published typing errors'; result.typingErrors = errors.map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') })); continue; }
    const engine = new ClassFootprints(), before = performance.now();
    for (const source of program.getSourceFiles().filter(source => !source.isDeclarationFile && source.fileName.startsWith(join(root, 'src') + '/'))) {
      const observations = classSnapshotFlows(program, source, engine);
      result.candidates.push(...observations.candidates.map(candidate => ({ ...candidate, path: source.fileName, consumerSha256: hash(source.text) })));
      result.openUses.push(...observations.refused); result.models.push(...observations.models);
    }
    result.analysisMs = performance.now() - before;
    if (row.id.startsWith('class-')) {
      assert.equal(result.candidates.length > 0, row.provenance.expectedCandidate, row.id);
      assert.equal(row.values.behavior.actual !== row.values.behavior.desired, row.provenance.expectedIssue, row.id + ': behavior');
    }
    console.log(`${row.id}: source candidates=${result.candidates.length}, open uses=${result.openUses.length}`);
  }
}
const summary = { consumers: results.length, refused: results.filter(row => row.refused).map(row => ({ id: row.id, reason: row.refused })),
  newTargets: results.filter(row => row.id.startsWith('class-') && row.provenance.expectedIssue).length,
  newTargetsWithCandidates: results.filter(row => row.id.startsWith('class-') && row.provenance.expectedIssue && row.candidates.length).length,
  newControls: results.filter(row => row.id.startsWith('class-') && !row.provenance.expectedIssue).length,
  newControlsWithCandidates: results.filter(row => row.id.startsWith('class-') && !row.provenance.expectedIssue && row.candidates.length).map(row => row.id),
  breadthTargetsWithCandidates: results.filter(row => row.id.startsWith('breadth-') && row.provenance.expectedIssue && row.candidates.length).map(row => row.id),
  breadthControlsWithCandidates: results.filter(row => row.id.startsWith('breadth-') && !row.provenance.expectedIssue && row.candidates.length).map(row => row.id) };
for (const input of frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.sha256);
writeFileSync(output, JSON.stringify({ authority: false, certification: false, basis: 'positive exact source footprint and snapshot flow; intent undeclared', frozen, summary, results, durationMs: performance.now() - started, finishedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
