import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { getterConsumerFlows } from './getter-paths.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [outputArgument, ...browserArguments] = process.argv.slice(2), output = resolve(outputArgument); assert(!existsSync(output));
const frozen = ['getter-paths.mjs', 'getter-path-cases.mjs', 'getter-path-study.mjs', 'source-extractor.mjs', 'lower.mjs', 'browser-experiment.mjs'].map(name => ({ name, sha256: hash(readFileSync(new URL(name, import.meta.url))) }));
const results = [];
for (const argument of browserArguments) {
  const path = resolve(argument), browser = read(path); assert(browser.finishedAt);
  for (const row of browser.results) {
    const root = join(dirname(path), row.id), main = join(root, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
    assert.equal(hash(readFileSync(main)), row.sourceSha256);
    const result = { id: row.id, provenance: row.provenance, browserPath: path, sourceSha256: row.sourceSha256, refused: null,
      notes: [], models: [], open: [], behavior: row.values?.behavior ?? null, errors: row.errors ?? [], typingErrors: row.publishedTypingErrors }; results.push(result);
    if (row.publishedTypingErrors.length) { result.refused = 'published typings rejected consumer'; continue; }
    if (row.runtime.some(runtime => runtime.version !== '2.0.0-rc.9')) { result.refused = 'runtime outside rc.9'; continue; }
    const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), root).options;
    const ambient = join(root, 'src/vite-env.d.ts'), program = ts.createProgram([main, ...(existsSync(ambient) ? [ambient] : [])], options);
    assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error), []);
    const before = performance.now();
    for (const source of program.getSourceFiles().filter(source => !source.isDeclarationFile && source.fileName.startsWith(join(root, 'src') + '/'))) {
      const observation = getterConsumerFlows(program, source);
      result.notes.push(...observation.notes.map(note => ({ ...note, path: source.fileName, consumerSha256: hash(source.text) })));
      result.models.push(...observation.models); result.open.push(...observation.open);
    }
    result.sourceMs = performance.now() - before;
    if (row.id.startsWith('getter-')) {
      assert.equal(result.notes.length > 0, !!row.provenance.expectedSourceNote, row.id);
      assert.equal(!!row.errors?.length, !!row.provenance.expectedException, row.id + ': exception');
      assert.equal(row.values.behavior.actual !== row.values.behavior.desired, !!row.provenance.expectedBehaviorFailure, row.id + ': displayed output');
      if (row.provenance.desiredDirect !== undefined) assert.equal(row.values.direct, row.provenance.desiredDirect);
    }
    console.log(`${row.id}: notes=${result.notes.length}, fields=${result.models.reduce((count, model) => count + model.fields.length, 0)}, open=${result.open.length}`);
  }
}
for (const input of frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.sha256);
const summary = { consumers: results.length, refused: results.filter(row => row.refused).map(row => ({ id: row.id, reason: row.refused })),
  newConsumers: results.filter(row => row.id.startsWith('getter-') && !row.refused).length,
  newAutomaticExceptions: results.filter(row => row.id.startsWith('getter-') && row.errors.length).map(row => row.id),
  newTargetsWithNotes: results.filter(row => row.id.startsWith('getter-') && row.provenance.expectedIssue && row.notes.length).map(row => row.id),
  newControlsWithNotes: results.filter(row => row.id.startsWith('getter-') && !row.provenance.expectedIssue && row.notes.length).map(row => row.id),
  breadthTargetsWithNotes: results.filter(row => row.id.startsWith('breadth-') && row.provenance.expectedIssue && row.notes.length).map(row => row.id),
  breadthControlsWithNotes: results.filter(row => row.id.startsWith('breadth-') && !row.provenance.expectedIssue && row.notes.length).map(row => row.id) };
writeFileSync(output, JSON.stringify({ authority: false, certification: false, typescript: ts.version, basis: 'finite positive source paths, not accepted package contracts', frozen, summary, results, finishedAt: new Date().toISOString() }, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
