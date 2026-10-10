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
    assert(recorded.returns.length > 0);
    const returns = recorded.returns.map(local => {
      const name = find(local.helperStart, undefined, node => ts.isIdentifier(node) || ts.isStringLiteral(node) || ts.isNumericLiteral(node)), returned = find(local.returnStart, local.returnEnd, ts.isExpression);
      assert(name && returned);
      const declaration = name.parent, fn = (ts.isFunctionDeclaration(declaration) || ts.isMethodDeclaration(declaration)) ? declaration : unwrap(declaration.initializer);
      assert(ts.isFunctionLike(fn)); assert(!fn.asteriskToken); assert(!fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword));
      assert(fn.parameters.every(parameter => ts.isIdentifier(parameter.name) && !parameter.initializer && !parameter.dotDotDotToken));
      const actualReturn = ts.isBlock(fn.body) ? fn.body.statements.length === 1 && ts.isReturnStatement(fn.body.statements[0]) && fn.body.statements[0].expression : fn.body;
      assert.equal(actualReturn, returned);
      const symbol = checker.getSymbolAtLocation(name); assert.equal(symbol.declarations.length, 1);
      return { local, name, returned, symbol, declaration: symbol.declarations[0] };
    });
    assert.equal(new Set(returns.map(item => item.declaration)).size, returns.length);
    function keyValue(expression, visited = new Set()) {
      const node = unwrap(expression);
      if (ts.isStringLiteral(node) || ts.isNumericLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
      assert(ts.isIdentifier(node)); const symbol = checker.getSymbolAtLocation(node);
      assert(symbol && symbol.declarations.length === 1 && !visited.has(symbol)); visited.add(symbol);
      const declaration = symbol.declarations[0]; assert(ts.isVariableDeclaration(declaration) && declaration.getSourceFile() === source && declaration.parent.flags & ts.NodeFlags.Const);
      return keyValue(declaration.initializer, visited);
    }
    function objectValue(expression, visited = new Set()) {
      const node = unwrap(expression); assert(ts.isIdentifier(node)); const symbol = checker.getSymbolAtLocation(node);
      assert(symbol && symbol.declarations.length === 1 && !visited.has(symbol)); visited.add(symbol);
      const declaration = symbol.declarations[0]; assert(ts.isVariableDeclaration(declaration) && declaration.getSourceFile() === source && declaration.parent.flags & ts.NodeFlags.Const);
      const value = unwrap(declaration.initializer);
      if (ts.isObjectLiteralExpression(value)) return { object: value, root: declaration, declarations: [declaration] };
      const inner = objectValue(value, visited); return { ...inner, declarations: [...inner.declarations, declaration] };
    }
    function stableObject(receiver) {
      assert(!receiver.object.properties.some(property => ts.isSpreadAssignment(property) || ts.isGetAccessor(property) || ts.isSetAccessor(property) || ts.isComputedPropertyName(property.name)));
      const scanThis = node => { assert.notEqual(node.kind, ts.SyntaxKind.ThisKeyword); ts.forEachChild(node, scanThis); }; scanThis(receiver.object);
      const pending = [...receiver.declarations], visited = new Set();
      while (pending.length) {
        const declaration = pending.pop(), symbol = checker.getSymbolAtLocation(declaration.name);
        if (visited.has(symbol)) continue; visited.add(symbol);
        for (const reference of nodes.filter(node => ts.isIdentifier(node) && node !== declaration.name &&
          (ts.isShorthandPropertyAssignment(node.parent) ? checker.getShorthandAssignmentValueSymbol(node.parent) : checker.getSymbolAtLocation(node)) === symbol)) {
          let value = reference; while (value.parent && unwrap(value.parent) === unwrap(value)) value = value.parent;
          const parent = value.parent;
          if (ts.isVariableDeclaration(parent) && parent.initializer === value && ts.isIdentifier(parent.name) && parent.parent.flags & ts.NodeFlags.Const) { pending.push(parent); continue; }
          assert((ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) && parent.expression === value && !parent.questionDotToken);
          if (ts.isElementAccessExpression(parent)) keyValue(parent.argumentExpression);
          let member = parent; while (member.parent && unwrap(member.parent) === unwrap(member)) member = member.parent;
          const use = member.parent;
          assert(ts.isCallExpression(use) && use.expression === member && !use.questionDotToken ||
            ts.isVariableDeclaration(use) && use.initializer === member && ts.isIdentifier(use.name) && use.parent.flags & ts.NodeFlags.Const);
        }
      }
    }
    function resolved(expression, seen = new Set(), aliases = [], members = []) {
      const value = unwrap(expression); assert(value);
      if (ts.isPropertyAccessExpression(value) || ts.isElementAccessExpression(value)) {
        assert(!value.questionDotToken); const receiver = objectValue(value.expression); stableObject(receiver);
        const key = ts.isPropertyAccessExpression(value) ? value.name.text : keyValue(value.argumentExpression), symbol = ts.isPropertyAccessExpression(value)
          ? checker.getSymbolAtLocation(value.name) : checker.getPropertyOfType(checker.getTypeAtLocation(value.expression), key);
        assert(symbol && symbol.declarations.length === 1 && !seen.has(symbol)); seen.add(symbol);
        const declaration = symbol.declarations[0]; assert.equal(declaration.parent, receiver.object);
        const edge = { start: value.getStart(source), end: value.end, receiverStart: receiver.root.name.getStart(source),
          propertyStart: declaration.getStart(source), propertyEnd: declaration.end }, nextMembers = [...members, edge];
        if (returns.some(item => item.declaration === declaration)) return { declaration, aliases, members: nextMembers };
        if (ts.isShorthandPropertyAssignment(declaration)) return resolved(declaration.name, seen, aliases, nextMembers);
        assert(ts.isPropertyAssignment(declaration)); return resolved(declaration.initializer, seen, aliases, nextMembers);
      }
      assert(ts.isIdentifier(value)); const symbol = ts.isShorthandPropertyAssignment(value.parent)
        ? checker.getShorthandAssignmentValueSymbol(value.parent) : checker.getSymbolAtLocation(value);
      assert(symbol && symbol.declarations.length === 1 && !seen.has(symbol)); seen.add(symbol);
      const declaration = symbol.declarations[0];
      if (returns.some(item => item.declaration === declaration)) return { declaration, aliases, members };
      assert(ts.isVariableDeclaration(declaration) && declaration.getSourceFile() === source && declaration.parent.flags & ts.NodeFlags.Const && ts.isIdentifier(declaration.name));
      return resolved(declaration.initializer, seen, [...aliases, { start: declaration.name.getStart(source), end: declaration.name.end }], members);
    }
    const leaf = unwrap(returns[0].returned); assert(ts.isPropertyAccessExpression(leaf) || ts.isElementAccessExpression(leaf));
    const pure = node => { assert(!ts.isCallExpression(node) && !ts.isNewExpression(node) && !ts.isFunctionLike(node) && !ts.isAwaitExpression(node)); ts.forEachChild(node, pure); }; pure(leaf);
    for (let index = 0; index < returns.length; index++) {
      const call = index + 1 === returns.length ? setupCall : unwrap(returns[index + 1].returned);
      assert(ts.isCallExpression(call) && !call.questionDotToken);
      const actual = resolved(call.expression); assert.equal(actual.declaration, returns[index].declaration);
      assert.deepEqual(actual.aliases, returns[index].local.aliases); assert.deepEqual(actual.members, returns[index].local.members ?? []);
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
  checks: ['real published declarations', 'exact helper declarations and immutable aliases', 'own member declarations and receiver mutation/escape audit', 'finite acyclic helper path', 'pure property return and exact returned call chain',
    'first recorded consumer frames outside call arguments', 'consumer and package source digests', 'recomputed package guard premise'], witnesses }, null, 2) + '\n');
console.log(JSON.stringify({ witnesses: witnesses.length, output }));
