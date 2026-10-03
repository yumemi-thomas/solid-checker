// Independently reconstruct recorded local return paths from published types.
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
    let recorded = candidate.observedPath;
    if (!recorded && candidate.observedCall) {
      const local = candidate.localReturn, edge = candidate.observedCall;
      recorded = { sourcePath: local.path, sourceSha256: local.sourceSha256, setupCall: { start: local.callStart, end: local.callEnd },
        returns: [{ ...local, aliases: [] }], frames: [{ frame: edge.returnFrame, index: edge.returnFrameIndex }, { frame: edge.callFrame, index: edge.callFrameIndex }] };
    }
    if (!recorded) continue;
    const text = readFileSync(path, 'utf8');
    assert.equal(recorded.sourcePath, path); assert.equal(recorded.sourceSha256, hash(text)); assert.equal(observed.sourceSha256, hash(text));
    const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
      { customConditions: ['browser', 'development'] }), allowJs: true }, root).options), source = program.getSourceFile(path), checker = program.getTypeChecker(), nodes = [];
    assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
    const scan = node => { nodes.push(node); ts.forEachChild(node, scan); }; scan(source);
    const find = (start, end, predicate) => nodes.find(node => node.getStart(source) === start && (end === undefined || node.end === end) && predicate(node));
    const setupCall = find(recorded.setupCall.start, recorded.setupCall.end, ts.isCallExpression); assert(setupCall);
    assert(recorded.returns.length > 0 && recorded.returns.length <= 4);
    const returns = recorded.returns.map(local => {
      const name = find(local.helperStart, undefined, ts.isIdentifier), returned = find(local.returnStart, local.returnEnd, ts.isExpression);
      assert(name && returned);
      const declaration = name.parent, fn = ts.isFunctionDeclaration(declaration) ? declaration : unwrap(declaration.initializer);
      assert(ts.isFunctionLike(fn)); assert(!fn.asteriskToken); assert(!fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword));
      assert(fn.parameters.every(parameter => ts.isIdentifier(parameter.name) && !parameter.initializer && !parameter.dotDotDotToken));
      const actualReturn = ts.isBlock(fn.body) ? fn.body.statements.length === 1 && ts.isReturnStatement(fn.body.statements[0]) && fn.body.statements[0].expression : fn.body;
      assert.equal(actualReturn, returned);
      const symbol = checker.getSymbolAtLocation(name); assert.equal(symbol.declarations.length, 1);
      return { local, name, returned, symbol };
    });
    function resolved(expression) {
      const aliases = [], seen = new Set();
      for (let value = unwrap(expression); value && ts.isIdentifier(value);) {
        const symbol = checker.getSymbolAtLocation(value); assert(symbol && !seen.has(symbol)); seen.add(symbol);
        if (returns.some(item => item.symbol === symbol)) return { symbol, aliases };
        assert.equal(symbol.declarations.length, 1); const declaration = symbol.declarations[0];
        assert(ts.isVariableDeclaration(declaration) && declaration.getSourceFile() === source && declaration.parent.flags & ts.NodeFlags.Const);
        assert(ts.isIdentifier(declaration.name)); assert(ts.isIdentifier(unwrap(declaration.initializer)));
        aliases.push({ start: declaration.name.getStart(source), end: declaration.name.end }); value = unwrap(declaration.initializer);
      }
      assert.fail('caller does not resolve through exact immutable identifiers');
    }
    const leaf = unwrap(returns[0].returned); assert(ts.isPropertyAccessExpression(leaf) || ts.isElementAccessExpression(leaf));
    const pure = node => { assert(!ts.isCallExpression(node) && !ts.isNewExpression(node) && !ts.isFunctionLike(node) && !ts.isAwaitExpression(node)); ts.forEachChild(node, pure); }; pure(leaf);
    for (let index = 0; index < returns.length; index++) {
      const call = index + 1 === returns.length ? setupCall : unwrap(returns[index + 1].returned);
      assert(ts.isCallExpression(call) && !call.questionDotToken);
      const actual = resolved(call.expression); assert.equal(actual.symbol, returns[index].symbol);
      assert.deepEqual(actual.aliases, returns[index].local.aliases);
    }
    assert.equal(recorded.frames.length, returns.length + 1);
    const matching = observed.guardTrace.find(event => event.path === candidate.premise.path && event.start === candidate.premise.start && event.end === candidate.premise.end &&
      event.sourceSha256 === candidate.premise.sourceSha256 && recorded.frames.every(({ frame, index }) => JSON.stringify(event.originalFrames[index]) === JSON.stringify(frame)));
    assert(matching); assert.equal(matching.kind, 'tracking-skipped');
    const appFrames = matching.originalFrames.map((frame, index) => ({ frame, index })).filter(({ frame }) => frame?.path === path);
    assert.deepEqual(appFrames.slice(0, recorded.frames.length), recorded.frames);
    for (let index = 0; index < recorded.frames.length; index++) {
      const frame = recorded.frames[index].frame; assert.equal(frame.path, path); assert.equal(frame.sourceSha256, hash(text));
      const offset = source.getPositionOfLineAndCharacter(frame.line - 1, frame.column - 1), expression = index === returns.length ? setupCall : returns[index].returned;
      assert(offset >= expression.getStart(source) && offset < expression.end);
      const value = unwrap(expression);
      if (ts.isCallExpression(value)) assert(![...value.arguments, ...value.typeArguments ?? []].some(argument => offset >= argument.getStart(source) && offset < argument.end));
    }
    const packageText = readFileSync(matching.path, 'utf8'); assert.equal(hash(packageText), matching.sourceSha256);
    assert(instrumentGuards(packageText, matching.path).observations.some(premise => premise.kind === matching.kind && premise.start === matching.start && premise.end === matching.end));
    witnesses.push({ id: row.id, path: recorded, packageGuard: candidate.premise });
  }
}
writeFileSync(output, JSON.stringify({ authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [studyPath, browserPath].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  auditor: { path: new URL(import.meta.url).pathname, sha256: hash(readFileSync(new URL(import.meta.url))) },
  checks: ['real published declarations', 'exact helper symbols and immutable aliases', 'pure property return and exact returned call chain',
    'first recorded consumer frames outside call arguments', 'consumer and package source digests', 'recomputed package guard premise'], witnesses }, null, 2) + '\n');
console.log(JSON.stringify({ witnesses: witnesses.length, output }));
