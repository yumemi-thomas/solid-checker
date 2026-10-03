import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { SourceExtractor } from './source-extractor.mjs';
import cases from './getter-path-cases.mjs';
const [studyArg, inventoryArg, getterArg, channelsArg, earlierChannelsArg, breadthArg, earlierBreadthArg, outputArg] = process.argv.slice(2);
const paths = [studyArg, inventoryArg, getterArg, channelsArg, earlierChannelsArg, breadthArg, earlierBreadthArg].map(path => resolve(path)), output = resolve(outputArg); assert(!existsSync(output));
const [study, inventory, getters, channels, earlierChannels, breadth, earlierBreadth] = paths.map(read);
for (const input of study.frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.sha256);
assert.equal(inventory.extractorSha256, hash(readFileSync(new URL('./getter-paths.mjs', import.meta.url))));
const runtimeErrors = row => [...(row.errors ?? []), ...(row.pageErrors ?? [])].map(error => error.message);
function authenticate(row, reportPath) {
  const root = join(dirname(reportPath), row.id), main = join(root, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
  assert.equal(hash(readFileSync(main)), row.sourceSha256); assert.equal(row.harnessFailure, undefined); assert.deepEqual(row.blockedRequests, []);
  for (const input of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, input.package)), input.pins);
  return root;
}
assert.equal(getters.results.length, cases.length);
for (const specimen of cases) {
  const row = getters.results.find(row => row.id === specimen.id), result = study.results.find(row => row.id === specimen.id); assert(row && result);
  const root = authenticate(row, paths[2]); assert.equal(row.sourceSha256, hash(specimen.source));
  if (specimen.provenance.expectedTypingCode) {
    assert.equal(row.excludedBeforeExecution, true); assert(row.publishedTypingErrors.some(error => error.code === specimen.provenance.expectedTypingCode));
    assert.equal(result.refused, 'published typings rejected consumer'); assert.deepEqual(result.notes, []); continue;
  }
  assert.deepEqual(row.publishedTypingErrors, []); assert.equal(row.disposals, 1);
  assert.equal(result.notes.length > 0, !!specimen.provenance.expectedSourceNote);
  assert.equal(row.errors.length > 0, !!specimen.provenance.expectedException);
  for (const error of row.errors) assert(error.originalLocation?.path.startsWith(root + '/src/'));
  for (const model of result.models) for (const source of model.sources) assert.equal(hash(readFileSync(source.path)), source.sha256);
  for (const note of result.notes) {
    assert.equal(note.severity, 'info'); assert.equal(note.certification, false);
    for (const read of note.reads) {
      const module = new SourceExtractor('browser').module(read.producer.path); let call = null;
      const visit = node => { if (ts.isCallExpression(node) && node.getStart(module.source) === read.producer.start) call = node; ts.forEachChild(node, visit); }; visit(module.source); assert(call);
      let imported = module.imports.get(module.checker.getSymbolAtLocation(call.expression));
      if (!imported && ts.isPropertyAccessExpression(call.expression)) {
        const receiver = module.imports.get(module.checker.getSymbolAtLocation(call.expression.expression));
        if (receiver?.namespace) imported = { specifier: receiver.specifier, name: call.expression.name.text };
      }
      assert(imported && !imported.namespace, 'producer must resolve the exact core import binding'); assert.equal(`${imported.specifier}.${imported.name}`, read.producer.native);
    }
  }
}
function graph(root) {
  const dir = join(root, '.vite-cache/deps'), metadataPath = join(dir, '_metadata.json'), metadata = read(metadataPath);
  const entries = ['solid-js', 'solid-js/attribution', '@solidjs/web'];
  // pnpm may expose signals only as Solid's transitive dependency. Its code
  // then belongs to the public entries' reachable chunks, not a root entry.
  for (const name of ['@solidjs/signals', '@solidjs/signals/attribution']) if (metadata.optimized[name]) entries.push(name);
  const files = new Map();
  function reachable(file, seen = new Set()) {
    file = resolve(file); assert(file.startsWith(dir + '/')); if (seen.has(file)) return seen; seen.add(file); assert(seen.size < 64);
    const text = readFileSync(file, 'utf8'); files.set(file, hash(text));
    const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS); assert.deepEqual(source.parseDiagnostics, []);
    for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
      assert(ts.isStringLiteral(node.moduleSpecifier)); assert(node.moduleSpecifier.text.startsWith('.'));
      reachable(resolve(dirname(file), node.moduleSpecifier.text), seen);
    }
    return seen;
  }
  const graphs = entries.map(name => { assert(metadata.optimized[name]); return reachable(join(dir, metadata.optimized[name].file)); });
  const shared = [...graphs[0]].filter(file => graphs.every(graph => graph.has(file))); assert(shared.length > 0);
  return { metadata: { path: metadataPath, sha256: hash(readFileSync(metadataPath)) }, entries, shared, files: [...files].map(([path, sha256]) => ({ path, sha256 })) };
}
const channelOutcomes = [], graphs = [];
for (const row of channels.results) {
  const previous = earlierChannels.results.find(previous => previous.id === row.id); assert(previous);
  const root = authenticate(row, paths[3]); assert.equal(row.sourceSha256, previous.sourceSha256);
  assert.deepEqual(row.publishedTypingErrors.map(error => error.code), previous.publishedTypingErrors.map(error => error.code));
  if (row.excludedBeforeExecution) continue;
  assert.equal(row.attributionPrebundle, true); assert.equal(row.attributionEnabled, true);
  assert.deepEqual(row.values, previous.values); assert.deepEqual(runtimeErrors(row), runtimeErrors(previous));
  assert.deepEqual(row.feedback.map(event => [event.code, event.severity]), previous.feedback.map(event => [event.code, event.severity]));
  const artifact = graph(root); graphs.push({ id: row.id, ...artifact });
  if (row.feedback.length) assert(row.feedback.some(event => event.frames.some(frame => frame.path.includes('/.vite-cache/deps/'))));
  channelOutcomes.push({ id: row.id, codes: row.feedback.map(event => event.code), engineInstalled: true, runs: row.attributionState.runs.length });
}
const breadthOutcomes = [], deliveryDifferences = [];
for (const row of breadth.results) {
  const previous = earlierBreadth.results.find(previous => previous.id === row.id); assert(previous);
  const root = authenticate(row, paths[5]); assert.equal(row.sourceSha256, previous.sourceSha256); assert.deepEqual(row.publishedTypingErrors, []);
  assert.deepEqual(row.values, previous.values);
  assert.deepEqual([...new Set(runtimeErrors(row))].sort(), [...new Set(runtimeErrors(previous))].sort());
  if (runtimeErrors(row).length !== runtimeErrors(previous).length) deliveryDifferences.push({ id: row.id, previous: runtimeErrors(previous).length, current: runtimeErrors(row).length });
  const semantic = row.feedback.filter(event => event.category !== 'advisory');
  assert.deepEqual([...new Set(semantic.map(event => event.code))].sort(), [...new Set(previous.feedback.map(event => event.code))].sort());
  const rc9 = row.runtime.every(runtime => runtime.version === '2.0.0-rc.9');
  assert.equal(row.attributionEnabled, rc9); assert.equal(row.attributionPrebundle, rc9);
  if (rc9) graphs.push({ id: row.id, ...graph(root) });
  if (!row.provenance.expectedIssue) { assert.deepEqual(row.feedback, []); assert.deepEqual(runtimeErrors(row), []); }
  breadthOutcomes.push({ id: row.id, expectedIssue: row.provenance.expectedIssue, codes: row.feedback.map(event => event.code), exceptions: runtimeErrors(row), engineInstalled: row.attributionEnabled });
}
const summary = { source: study.summary, inventory: inventory.summary,
  getterBrowserExecutions: getters.results.filter(row => !row.excludedBeforeExecution).length,
  prebundledChannelExecutions: channelOutcomes.length, prebundledBreadthExecutions: breadthOutcomes.length,
  optimizedGraphsValidated: graphs.length, sharedEngineInstalled: graphs.length,
  prebundledChannelTargetsDetected: channelOutcomes.filter(row => row.id.endsWith('-target') && row.codes.length).length,
  prebundledBreadthTargetsDirectlyDetected: breadthOutcomes.filter(row => row.expectedIssue && (row.codes.length || row.exceptions.length)).length,
  prebundledControlFeedback: breadthOutcomes.filter(row => !row.expectedIssue && (row.codes.length || row.exceptions.length)).length,
  exceptionDeliveryParity: deliveryDifferences.length === 0, deliveryDifferences,
  sourceOutputChangesRequireIntent: true, fullPackageCoverageProven: false };
writeFileSync(output, JSON.stringify({ authority: false, certification: false, validatorSha256: hash(readFileSync(new URL(import.meta.url))), typescript: ts.version,
  inputs: paths.map(path => ({ path, sha256: hash(readFileSync(path)) })), summary, channelOutcomes, breadthOutcomes, graphs, verifiedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
