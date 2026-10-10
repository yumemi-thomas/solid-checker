import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import cases from './lifetime-all-cases.mjs';
const [finalArg, inventoryArg, ...baselineAndOutput] = process.argv.slice(2), outputArg = baselineAndOutput.pop();
const [finalDir, inventoryPath, output] = [finalArg, inventoryArg, outputArg].map(path => resolve(path)), baselinePaths = baselineAndOutput.map(path => resolve(path)); assert(!existsSync(output));
const before = read(join(finalDir, 'inputs-before.json')), after = read(join(finalDir, 'inputs-after.json'));
assert.deepEqual(before.files, after.files); for (const input of before.files) assert.equal(hash(readFileSync(input.path)), input.sha256);
const finalPath = join(finalDir, 'browser/results.json'), final = read(finalPath), baselines = baselinePaths.flatMap(path => read(path).results.map(row => ({ row, reportPath: path })));
assert(final.finishedAt); assert.equal(final.results.length, cases.length); assert.equal(baselines.length, cases.length);
assert.equal(final.bridgeSha256, hash(readFileSync(new URL('./lifetime-feedback.mjs', import.meta.url))));
function authenticate(row, reportPath, specimen) {
  const root = join(dirname(reportPath), row.id); assert.equal(hash(readFileSync(join(root, 'src/main.tsx'))), row.sourceSha256); assert.equal(row.sourceSha256, hash(specimen.source));
  assert.deepEqual(row.publishedTypingErrors, []); assert.equal(row.excludedBeforeExecution, undefined); assert.equal(row.harnessFailure, undefined);
  assert.deepEqual(row.blockedRequests, []); assert.deepEqual(row.errors, []); assert.deepEqual(row.pageErrors, []); assert.deepEqual(row.windowErrors, []); assert.equal(row.disposals, 1);
  for (const runtime of row.runtime) assert.equal(read(join(runtime.dir, 'package.json')).version, '2.0.0-rc.9');
  for (const input of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, input.package)), input.pins);
  assert.equal(row.attributionPrebundle, true); assert.equal(row.attributionEnabled, true); return root;
}
function optimized(root) {
  const dir = join(root, '.vite-cache/deps'), metadataPath = join(dir, '_metadata.json'), metadata = read(metadataPath), names = ['solid-js', 'solid-js/attribution', '@solidjs/web'];
  for (const name of ['@solidjs/signals', '@solidjs/signals/attribution']) if (metadata.optimized[name]) names.push(name);
  const files = new Map();
  function reachable(path, seen = new Set()) {
    path = resolve(path); assert(path.startsWith(dir + '/')); if (seen.has(path)) return seen; seen.add(path); assert(seen.size < 64);
    const text = readFileSync(path, 'utf8'); files.set(path, hash(text)); const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS); assert.deepEqual(source.parseDiagnostics, []);
    for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
      assert(ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text.startsWith('.')); reachable(resolve(dirname(path), node.moduleSpecifier.text), seen);
    }
    return seen;
  }
  const graphs = names.map(name => { assert(metadata.optimized[name]); return reachable(join(dir, metadata.optimized[name].file)); });
  const shared = [...graphs[0]].filter(path => graphs.every(graph => graph.has(path))); assert(shared.length);
  return { metadata: { path: metadataPath, sha256: hash(readFileSync(metadataPath)) }, shared, files: [...files].map(([path, sha256]) => ({ path, sha256 })) };
}
const code = 'RESOURCE_OUTLIVES_DECLARED_SCOPE', results = [], graphs = [];
for (const specimen of cases) {
  const row = final.results.find(row => row.id === specimen.id), baseline = baselines.find(entry => entry.row.id === specimen.id); assert(row && baseline);
  const root = authenticate(row, finalPath, specimen); authenticate(baseline.row, baseline.reportPath, specimen); graphs.push({ id: row.id, ...optimized(root) });
  const warnings = row.feedback.filter(event => event.code === code), core = row.feedback.filter(event => event.code !== code);
  assert.deepEqual(core.map(event => [event.code, event.severity]), baseline.row.feedback.map(event => [event.code, event.severity]));
  const detected = specimen.provenance.expectedIssue && !specimen.provenance.expectedMiss; assert.equal(warnings.length, detected ? 1 : 0);
  if (warnings.length) {
    const warning = warnings[0]; assert.equal(warning.resourceKind, specimen.provenance.expectedKind); assert.equal(warning.certification, false); assert.equal(warning.category, 'lifetime-expectation');
    assert(warning.originalLocation?.path.startsWith(root + '/src/')); assert(warning.frames.some(frame => frame.path.includes('/node_modules/')));
  }
  const actual = row.values, previous = baseline.row.values;
  let behaviorParity;
  if (specimen.provenance.measuresCost) {
    for (const value of [actual, previous]) { assert.equal(value.operationsPerSample, 800); assert.equal(value.samples.length, 9); assert(value.samples.every(time => Number.isFinite(time) && time >= 0)); }
    behaviorParity = true;
  } else if (row.id.includes('manual-interval') || row.id.includes('restart')) {
    const survives = value => (value.calls ?? 0) > (value.before ?? 0);
    assert.equal(survives(actual), specimen.provenance.expectedIssue); assert.equal(survives(previous), specimen.provenance.expectedIssue); behaviorParity = true;
  } else {
    const strip = value => { const { resourceAudit, ...rest } = value; return rest; }; assert.deepEqual(strip(actual), strip(previous)); behaviorParity = true;
    if (specimen.provenance.expectedIssue) assert.equal(actual.calls, 1);
  }
  const snapshot = actual.resourceAudit; assert(snapshot); assert.equal(snapshot.feedback.length, warnings.length);
  assert.equal(snapshot.gaps.length > 0, !!specimen.provenance.expectedGap);
  if (specimen.provenance.expectedGaps) assert.equal(snapshot.gaps.length, specimen.provenance.expectedGaps);
  results.push({ id: row.id, package: specimen.package, role: specimen.provenance.role, expectedIssue: specimen.provenance.expectedIssue, expectedMiss: specimen.provenance.expectedMiss ?? false,
    warnings: warnings.map(event => ({ code: event.code, resourceKind: event.resourceKind, originalLocation: event.originalLocation })),
    coreCodes: core.map(event => event.code), behaviorParity, gapReasons: snapshot.gaps.map(gap => gap.reason), activeAtObservation: snapshot.active.length });
}
const inventory = read(inventoryPath); assert.equal(inventory.scriptSha256, hash(readFileSync(new URL('./lifetime-platform-inventory.mjs', import.meta.url))));
const modules = new Map();
for (const row of inventory.results) {
  if (row.refused) { assert.deepEqual(row.sites, []); continue; }
  assert.equal(row.version, row.pins.find(pin => pin.package === row.package)?.version);
  for (const input of row.modules) assert.equal(hash(readFileSync(input.path)), input.sha256);
  for (const site of row.sites) {
    if (!modules.has(site.source.path)) {
      const program = ts.createProgram([site.source.path], { allowJs: true, noResolve: true, target: ts.ScriptTarget.ESNext }); modules.set(site.source.path, { source: program.getSourceFile(site.source.path), checker: program.getTypeChecker() });
    }
    const { source, checker } = modules.get(site.source.path); let call;
    function visit(node) { if ((ts.isCallExpression(node) || ts.isNewExpression(node)) && node.getStart(source) === site.source.start) call = node; ts.forEachChild(node, visit); } visit(source);
    assert(call); assert.equal(call.getText(source), site.source.text); const symbol = checker.getSymbolAtLocation(ts.isPropertyAccessExpression(call.expression) ? call.expression.name : call.expression);
    assert(symbol?.declarations?.length && symbol.declarations.every(declaration => declaration.getSourceFile().fileName.endsWith('/lib.dom.d.ts')));
    const declaration = symbol.declarations.find(declaration => declaration.getStart() === site.declaration.start && declaration.getSourceFile().fileName === site.declaration.path); assert(declaration);
    assert.equal(hash(readFileSync(site.declaration.path)), site.declaration.sha256);
  }
}
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)], costRow = final.results.find(row => row.id === 'lifetime-operation-cost-control'), costBase = baselines.find(entry => entry.row.id === costRow.id).row;
const cost = { operationsPerSample: 800, registrationsPerSample: 400, samples: 9, originalMedianMs: median(costBase.values.samples), monitoredMedianMs: median(costRow.values.samples), basis: 'one local warm browser microbenchmark, not application latency' };
cost.addedMicrosecondsPerRegistration = (cost.monitoredMedianMs - cost.originalMedianMs) / 400 * 1000;
const summary = { consumers: results.length, targets: results.filter(row => row.expectedIssue).length, targetsWithLifetimeWarnings: results.filter(row => row.warnings.length).length,
  missedTargets: results.filter(row => row.expectedMiss).map(row => row.id), controls: results.filter(row => !row.expectedIssue).length, newControlLifetimeWarnings: results.filter(row => !row.expectedIssue).reduce((sum, row) => sum + row.warnings.length, 0),
  controlsWithSharedCoreFeedback: results.filter(row => !row.expectedIssue && row.coreCodes.length).map(row => ({ id: row.id, codes: row.coreCodes })),
  optimizedGraphs: graphs.length, lifetimePackagesExecuted: [...new Set(cases.map(row => row.package))], exactPlatformReferenceInventory: inventory.summary, cost,
  automaticLifetimeIntentInferred: false, fullPackageCoverageProven: false };
const inputs = [finalPath, inventoryPath, ...baselinePaths, join(finalDir, 'inputs-before.json'), join(finalDir, 'inputs-after.json')];
writeFileSync(output, JSON.stringify({ authority: false, certification: false, validatorSha256: hash(readFileSync(new URL(import.meta.url))),
  inputs: inputs.map(path => ({ path, sha256: hash(readFileSync(path)) })), summary, results, graphs, finishedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify({ ...summary, exactPlatformReferenceInventory: { packages: inventory.summary.packages, packagesWithExactPlatformReferences: inventory.summary.packagesWithExactPlatformReferences, sites: inventory.summary.sites, refused: inventory.summary.refused } }, null, 2));
