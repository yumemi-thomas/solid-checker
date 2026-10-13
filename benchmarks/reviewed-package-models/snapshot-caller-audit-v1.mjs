// Independently inspect the retained caller witnesses using real declarations.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [studyArg, browserArg, outputArg] = process.argv.slice(2), studyPath = resolve(studyArg), browserPath = resolve(browserArg), output = resolve(outputArg),
  study = read(studyPath), browser = read(browserPath), witnesses = [];
assert(!existsSync(output)); assert.equal(study.certification, false);
assert(study.inputs.some(pin => pin.path === browserPath && pin.sha256 === hash(readFileSync(browserPath))));
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
for (const row of study.challenges) {
  const observed = browser.results.find(item => item.id === row.id), root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx');
  for (const candidate of row.observedGetters?.candidates ?? []) {
    if (!candidate.observedCall) continue;
    const text = readFileSync(path, 'utf8'), local = candidate.localReturn, edge = candidate.observedCall;
    assert.equal(local.path, path); assert.equal(local.sourceSha256, hash(text)); assert.equal(observed.sourceSha256, hash(text));
    const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
      { customConditions: ['browser', 'development'] }), allowJs: true }, root).options), source = program.getSourceFile(path), checker = program.getTypeChecker();
    assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
    let call, returned, name;
    const scan = node => {
      if (ts.isCallExpression(node) && node.getStart(source) === local.callStart && node.end === local.callEnd) call = node;
      if (node.getStart(source) === local.returnStart && node.end === local.returnEnd) returned = node;
      if (ts.isIdentifier(node) && node.getStart(source) === local.helperStart) name = node;
      ts.forEachChild(node, scan);
    }; scan(source);
    assert(call && returned && name); assert.equal(call.arguments.length, 0); assert(!call.questionDotToken);
    const callee = unwrap(call.expression); assert(ts.isIdentifier(callee));
    assert.equal(checker.getSymbolAtLocation(callee), checker.getSymbolAtLocation(name));
    const declaration = name.parent, fn = ts.isFunctionDeclaration(declaration) ? declaration : unwrap(declaration.initializer);
    assert(ts.isFunctionLike(fn)); assert.equal(fn.parameters.length, 0);
    assert(returned.getStart(source) >= fn.body.getStart(source) && returned.end <= fn.body.end);
    assert(ts.isPropertyAccessExpression(unwrap(returned)) || ts.isElementAccessExpression(unwrap(returned)));
    const offset = frame => {
      assert.equal(frame.path, path); assert.equal(frame.sourceSha256, hash(text));
      return source.getPositionOfLineAndCharacter(frame.line - 1, frame.column - 1);
    }, returnedPosition = offset(edge.returnFrame), callerPosition = offset(edge.callFrame);
    assert(returnedPosition >= local.returnStart && returnedPosition < local.returnEnd);
    assert(callerPosition >= call.getStart(source) && callerPosition < call.end);
    const matching = observed.guardTrace.find(event => event.path === candidate.premise.path && event.start === candidate.premise.start && event.end === candidate.premise.end &&
      event.sourceSha256 === candidate.premise.sourceSha256 && JSON.stringify(event.originalFrames[edge.returnFrameIndex]) === JSON.stringify(edge.returnFrame) &&
      JSON.stringify(event.originalFrames[edge.callFrameIndex]) === JSON.stringify(edge.callFrame));
    assert(matching); assert.equal(matching.kind, 'tracking-skipped');
    const appFrames = matching.originalFrames.map((frame, index) => ({ frame, index })).filter(({ frame }) => frame?.path === path);
    assert.equal(appFrames[0].index, edge.returnFrameIndex); assert.equal(appFrames[1].index, edge.callFrameIndex);
    const packageText = readFileSync(matching.path, 'utf8'); assert.equal(hash(packageText), matching.sourceSha256);
    assert(instrumentGuards(packageText, matching.path).observations.some(premise => premise.kind === matching.kind && premise.start === matching.start && premise.end === matching.end));
    witnesses.push({ id: row.id, helperDeclaration: { path, start: name.getStart(source) }, call: { start: call.getStart(source), end: call.end },
      returned: { start: returned.getStart(source), end: returned.end }, packageGuard: candidate.premise, consumerSourceSha256: hash(text) });
  }
}
writeFileSync(output, JSON.stringify({ authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [studyPath, browserPath].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  auditor: { path: new URL(import.meta.url).pathname, sha256: hash(readFileSync(new URL(import.meta.url))) },
  checks: ['real published declarations', 'exact helper symbol and declaration', 'return and direct call spans', 'recorded first two consumer frames',
    'consumer and package source digests', 'recomputed installed package guard premise'], witnesses }, null, 2) + '\n');
console.log(JSON.stringify({ witnesses: witnesses.length, output }));
