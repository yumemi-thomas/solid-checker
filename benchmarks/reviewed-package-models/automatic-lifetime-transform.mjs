import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const runtime = new URL('./automatic-lifetime-runtime.mjs', import.meta.url).pathname;
const sample = ts.transpileModule('async function __probe(v) { await v; }', { compilerOptions: { target: ts.ScriptTarget.ES2016, module: ts.ModuleKind.ESNext } }).outputText;
const parsed = text => ts.createSourceFile('emitted.js', text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
const sampleHelper = parsed(sample).statements.find(node => ts.isVariableStatement(node) && node.declarationList.declarations.some(declaration => declaration.name.text === '__awaiter')).declarationList.declarations[0].initializer.getText();
function instrumentHelper(code) {
  const source = parsed(code), helpers = [];
  for (const statement of source.statements) if (ts.isVariableStatement(statement)) for (const declaration of statement.declarationList.declarations) {
    if (declaration.name.text === '__awaiter' && declaration.initializer?.getText(source) === sampleHelper) helpers.push(declaration);
  }
  if (!helpers.length) return { code, helper: null };
  assert.equal(helpers.length, 1); const helper = helpers[0], fn = helper.initializer.right, edits = [];
  assert(ts.isFunctionExpression(fn));
  edits.push({ start: fn.body.getStart(source) + 1, end: fn.body.getStart(source) + 1, text: ' const __resourceResume = globalThis.__resourceAudit?.capture() ?? null;' });
  function visit(node) {
    if (ts.isCallExpression(node) && (ts.isPropertyAccessExpression(node.expression) || ts.isElementAccessExpression(node.expression))) {
      const member = ts.isPropertyAccessExpression(node.expression) ? node.expression.name.text : node.expression.argumentExpression.text;
      const receiver = node.expression.expression;
      const direct = ts.isIdentifier(receiver) && receiver.text === 'generator';
      const initial = ts.isParenthesizedExpression(receiver) && ts.isBinaryExpression(receiver.expression) && receiver.expression.left.getText(source) === 'generator';
      if (['next', 'throw'].includes(member) && (direct || initial)) {
        const text = node.getText(source); edits.push({ start: node.getStart(source), end: node.end, text: `(__resourceResume ? __resourceResume.run(() => ${text}) : ${text})` }); return;
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(fn.body); assert.equal(edits.length, 4, 'Pinned TypeScript awaiter must have three resumption operations');
  // Edits stay inside the unmapped emitted helper and add no new lines. Source
  // mappings for the original consumer body keep their line/column positions.
  for (const edit of edits.sort((a, b) => b.start - a.start)) code = code.slice(0, edit.start) + edit.text + code.slice(edit.end);
  return { code, helper: { compiler: ts.version, templateSha256: hash(sampleHelper), resumptions: 3 } };
}
export function transformLifetime(code, path, { asyncContext = false, nativeContext = false } = {}) {
  assert(!(asyncContext && nativeContext), 'Choose one async instrumentation profile');
  const root = dirname(path), web = packageRoot(root, '@solidjs/web');
  for (const runtime of nativeRuntimeRoots(root)) assert.equal(read(join(runtime, 'package.json')).version, '2.0.0-rc.9');
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), allowJs: true, jsxImportSource: '@solidjs/web' }, root).options;
  const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
  const scriptKind = path.endsWith('.tsx') ? ts.ScriptKind.TSX : path.endsWith('.jsx') ? ts.ScriptKind.JSX : path.endsWith('.ts') ? ts.ScriptKind.TS : ts.ScriptKind.JS;
  host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true, scriptKind) : get(file, ...args);
  host.fileExists = file => file === path || exists(file);
  const program = ts.createProgram([path], options, host), source = program.getSourceFile(path), checker = program.getTypeChecker();
  assert(source && !source.parseDiagnostics.length);
  const names = new Set(), events = new Map(), gaps = []; let unsupportedAsync = false;
  function inspect(node) {
    if (ts.isIdentifier(node)) names.add(node.text);
    if (ts.isForOfStatement(node) && node.awaitModifier || ts.isFunctionLike(node) && node.asteriskToken && node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword)) unsupportedAsync = true;
    if (ts.isJsxAttribute(node) && ts.isJsxExpression(node.initializer) && node.initializer.expression) {
      const tag = node.parent.parent.tagName, tagSymbol = checker.getSymbolAtLocation(tag);
      const tagDeclarations = tagSymbol?.declarations ?? [];
      const intrinsic = tagDeclarations.length && tagDeclarations.every(declaration => declaration.getSourceFile().fileName === join(web, 'types/jsx.d.ts'));
      if (intrinsic) {
        const type = checker.getTypeOfSymbolAtLocation(tagSymbol, tag), symbol = type.getProperty(node.name.getText(source));
        const declarations = symbol?.declarations ?? [];
        const event = declarations.length && declarations.every(declaration => declaration.getSourceFile().fileName === join(web, 'types/jsx.d.ts') && declaration.parent.name?.text === 'EventHandlersElement');
        if (event) {
          const location = source.getLineAndCharacterOfPosition(node.initializer.expression.getStart(source));
          events.set(`${node.initializer.expression.getStart(source)}:${node.initializer.expression.end}`, { path, start: node.initializer.expression.getStart(source), end: node.initializer.expression.end, line: location.line + 1, column: location.character + 1,
            tag: tag.getText(source), property: node.name.getText(source), sourceSha256: hash(code), declarations: declarations.map(declaration => ({ path: declaration.getSourceFile().fileName, start: declaration.getStart(), sha256: hash(declaration.getSourceFile().text) })) });
        }
      }
    }
    ts.forEachChild(node, inspect);
  }
  inspect(source);
  if (asyncContext && unsupportedAsync) { gaps.push({ path, reason: 'Async generator or for-await lowering not admitted' }); asyncContext = false; }
  let prefix = '__resourceLifetime'; while ([...names].some(name => name.startsWith(prefix))) prefix += '_';
  const factory = ts.factory, id = suffix => factory.createIdentifier(prefix + suffix);
  const metadata = [...events.values()], continuations = []; let captureIndex = 0;
  const emitted = ts.transpileModule(code, { fileName: path, compilerOptions: { target: asyncContext ? ts.ScriptTarget.ES2016 : ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
    jsx: ts.JsxEmit.Preserve, sourceMap: true, inlineSources: true }, transformers: { before: [context => root => {
      let capture = null;
      function contains(node, predicate, skipFunctions = true) {
        if (predicate(node)) return true;
        if (skipFunctions && ts.isFunctionLike(node)) return false;
        let found = false; ts.forEachChild(node, child => { if (!found) found = contains(child, predicate, skipFunctions); }); return found;
      }
      function preserve(next, node) { ts.setOriginalNode(next, node); return ts.setTextRange(next, node); }
      function visit(node, bindEvent = true) {
        const key = `${node.getStart(root)}:${node.end}`;
        if (bindEvent && events.has(key)) {
          return preserve(factory.createCallExpression(id('Event'), undefined, [visit(node, false), id('Owner'), id('Cleanup'), factory.createStringLiteral(JSON.stringify(events.get(key)))]), node);
        }
        if (nativeContext && ts.isFunctionLike(node)) {
          const previous = capture, async = node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword);
          if (!async && previous && node.body) gaps.push({ path, start: node.getStart(root), reason: 'Ordinary nested function has no automatic continuation context' });
          let admitted = async && !!node.body && !node.asteriskToken;
          if (admitted && contains(node.body, child => ts.isForOfStatement(child) && child.awaitModifier || ts.isCallExpression(child) && ts.isIdentifier(child.expression) && child.expression.text === 'eval')) {
            admitted = false; gaps.push({ path, start: node.getStart(root), reason: 'Direct eval or for-await function remains native and uninstrumented' });
          }
          if (async && node.asteriskToken) gaps.push({ path, start: node.getStart(root), reason: 'Async generator remains uninstrumented' });
          const local = admitted ? id('Context' + captureIndex++) : null;
          capture = null;
          const next = ts.visitEachChild(node, child => {
            if (child !== node.body || !admitted) return visit(child);
            capture = local; const body = visit(child); capture = null;
            const value = factory.createCallExpression(id('Capture'), undefined, []);
            const statement = factory.createVariableStatement(undefined, factory.createVariableDeclarationList([factory.createVariableDeclaration(local, undefined, undefined, value)], ts.NodeFlags.Const));
            if (!ts.isBlock(body)) return factory.createBlock([statement, factory.createReturnStatement(body)], true);
            let directives = 0; while (body.statements[directives] && ts.isExpressionStatement(body.statements[directives]) && ts.isStringLiteral(body.statements[directives].expression)) directives++;
            return factory.updateBlock(body, [...body.statements.slice(0, directives), statement, ...body.statements.slice(directives)]);
          }, context);
          capture = previous;
          if (admitted) continuations.push({ path, start: node.getStart(root), end: node.end });
          return next;
        }
        if (capture && (ts.isCallExpression(node) || ts.isNewExpression(node))) {
          if (node.flags & ts.NodeFlags.OptionalChain) {
            gaps.push({ path, start: node.getStart(root), reason: 'Optional call chain remains uninstrumented' }); return node;
          }
          if (contains(node, child => ts.isAwaitExpression(child) || ts.isYieldExpression(child))) {
            // Bind the callee before evaluating arguments; awaits remain in
            // their native function. A member bridge retains its receiver.
            if (!contains(node, child => ts.isYieldExpression(child)) && ts.isCallExpression(node) && (ts.isIdentifier(node.expression) || ts.isCallExpression(node.expression))) {
              const callee = visit(node.expression), args = node.arguments.map(argument => visit(argument));
              const bound = factory.createParenthesizedExpression(factory.createConditionalExpression(capture, undefined,
                factory.createCallExpression(factory.createPropertyAccessExpression(capture, 'bind'), undefined, [callee]), undefined, callee));
              return preserve(factory.updateCallExpression(node, bound, node.typeArguments, args), node);
            }
            if (!contains(node, child => ts.isYieldExpression(child)) && ts.isCallExpression(node) &&
              (ts.isPropertyAccessExpression(node.expression) || ts.isElementAccessExpression(node.expression)) &&
              node.expression.expression.kind !== ts.SyntaxKind.SuperKeyword && !(ts.isPropertyAccessExpression(node.expression) && ts.isPrivateIdentifier(node.expression.name))) {
              const target = visit(node.expression.expression), property = ts.isPropertyAccessExpression(node.expression) ? factory.createStringLiteral(node.expression.name.text) : visit(node.expression.argumentExpression);
              const original = ts.visitEachChild(node.expression, child => visit(child), context);
              const bound = factory.createParenthesizedExpression(factory.createConditionalExpression(capture, undefined,
                factory.createCallExpression(factory.createPropertyAccessExpression(capture, 'member'), undefined, [target, property]), undefined, original));
              return preserve(factory.updateCallExpression(node, bound, node.typeArguments, node.arguments.map(argument => visit(argument))), node);
            }
            gaps.push({ path, start: node.getStart(root), reason: 'Call with awaited or yielded operand remains uninstrumented' });
            return ts.visitEachChild(node, child => visit(child), context);
          }
          const operation = ts.visitEachChild(node, child => visit(child), context);
          return preserve(factory.createParenthesizedExpression(factory.createConditionalExpression(capture, undefined,
            factory.createCallExpression(factory.createPropertyAccessExpression(capture, 'run'), undefined, [factory.createArrowFunction(undefined, undefined, [], undefined, factory.createToken(ts.SyntaxKind.EqualsGreaterThanToken), operation)]), undefined, operation)), node);
        }
        return ts.visitEachChild(node, child => visit(child), context);
      }
      let next = ts.visitNode(root, visit);
      if (events.size || continuations.length) {
        const imports = [...(events.size ? [factory.createImportDeclaration(undefined, factory.createImportClause(false, undefined, factory.createNamedImports([
          factory.createImportSpecifier(false, factory.createIdentifier('getOwner'), id('Owner')), factory.createImportSpecifier(false, factory.createIdentifier('onCleanup'), id('Cleanup')),
        ])), factory.createStringLiteral('solid-js'))] : []),
        factory.createImportDeclaration(undefined, factory.createImportClause(false, undefined, factory.createNamedImports([
          ...(events.size ? [factory.createImportSpecifier(false, factory.createIdentifier('ownerEvent'), id('Event'))] : []),
          ...(continuations.length ? [factory.createImportSpecifier(false, factory.createIdentifier('captureLifetime'), id('Capture'))] : []),
        ])), factory.createStringLiteral('/@fs' + runtime))];
        next = factory.updateSourceFile(next, [...imports, ...next.statements]);
      }
      return next;
    }] } });
  const result = asyncContext ? instrumentHelper(emitted.outputText) : { code: emitted.outputText, helper: null };
  return { code: result.code, map: JSON.parse(emitted.sourceMapText), metadata, helper: result.helper, continuations, gaps };
}
export default function lifetimePlugin() {
  const transformed = [], refused = []; let project;
  return { name: 'explicit-project-event-lifetime', enforce: 'pre', transformed, refused,
    configResolved(config) { project = config.root; },
    transform(code, id) {
      const path = id.split('?')[0]; if (!path.startsWith(project + '/src/') || !/\.[jt]sx?$/.test(path)) return null;
      try {
        const result = transformLifetime(code, path, { asyncContext: process.env.REVIEWED_MODEL_ASYNC_CONTEXT === '1', nativeContext: process.env.REVIEWED_MODEL_NATIVE_CONTEXT === '1' });
        transformed.push({ path, sourceSha256: hash(code), transformedSha256: hash(result.code), events: result.metadata, helper: result.helper, continuations: result.continuations, gaps: result.gaps });
        return { code: result.code, map: result.map };
      } catch (error) { refused.push({ path, reason: error.message }); return null; }
    } };
}
