// Informational snapshot feedback, built on the frozen positive-footprint engine.
// No observer footprint is promoted to a proven reactive defect.
import { existsSync, readFileSync } from 'node:fs';
import { ts } from './lower.mjs';
import { hash } from './catalog.mjs';
import { ClassFootprints, classSnapshotFlows } from './class-footprints.mjs';
import { familyFeedback } from './family-feedback-system.mjs';
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const unalias = (checker, symbol) => symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
export function exactComputedMembers(program, source) {
  const checker = program.getTypeChecker(), edits = [], evidence = [], refused = [];
  const visit = node => {
    if (ts.isElementAccessExpression(node) && !node.questionDotToken) {
      const argument = unwrap(node.argumentExpression), receiver = unwrap(node.expression);
      let key = ts.isStringLiteral(argument) ? argument.text : null;
      if (ts.isIdentifier(argument)) {
        const declaration = checker.getSymbolAtLocation(argument)?.valueDeclaration;
        if (declaration && ts.isVariableDeclaration(declaration) && declaration.parent.flags & ts.NodeFlags.Const &&
          ts.isStringLiteral(unwrap(declaration.initializer))) key = unwrap(declaration.initializer).text;
      }
      if (key !== null && ts.isIdentifier(receiver) && ts.isIdentifierText(key, ts.ScriptTarget.Latest)) {
        const declared = checker.getSymbolAtLocation(receiver)?.valueDeclaration,
          member = checker.getTypeAtLocation(receiver).getProperty(key),
          resolved = unalias(checker, checker.getSymbolAtLocation(node.argumentExpression));
        // A plain const instance, a concrete key and the exact declared property.
        // Type-only assertions cannot replace these runtime premises.
        const stable = declared && ts.isVariableDeclaration(declared) && declared.parent.flags & ts.NodeFlags.Const &&
          ts.isNewExpression(unwrap(declared.initializer));
        const exact = member?.declarations?.length && (!ts.isStringLiteral(argument) || resolved === member);
        const start = node.expression.end, end = node.end, suffix = source.text.slice(start, end), replacement = '.' + key;
        if (stable && exact && !suffix.includes('\n') && !suffix.includes('\r') && replacement.length <= suffix.length) {
          edits.push({ start, end, text: replacement.padEnd(suffix.length) });
          evidence.push({ start: node.getStart(source), end: node.end, member: key,
            declarations: member.declarations.map(declaration => ({ path: declaration.getSourceFile().fileName,
              start: declaration.getStart(), end: declaration.end })) });
        } else refused.push({ start: node.getStart(source), reason: 'computed member lacks stable construction and exact declaration' });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source); let text = source.text;
  for (const edit of edits.sort((a, b) => b.start - a.start)) text = text.slice(0, edit.start) + edit.text + text.slice(edit.end);
  if (!edits.length) return { program, source, evidence, refused };
  const options = program.getCompilerOptions(), host = ts.createCompilerHost(options), original = host.getSourceFile.bind(host);
  host.getSourceFile = (path, languageVersion, onError, shouldCreateNewSourceFile) => path === source.fileName
    ? ts.createSourceFile(path, text, languageVersion, true, ts.ScriptKind.TSX)
    : original(path, languageVersion, onError, shouldCreateNewSourceFile);
  const normalized = ts.createProgram(program.getRootFileNames(), options, host), normalizedSource = normalized.getSourceFile(source.fileName);
  return { program: normalized, source: normalizedSource, evidence, refused };
}
function constructorEscapes(program, source) {
  const checker = program.getTypeChecker(), constructors = new Map();
  const collect = node => {
    if (ts.isNewExpression(node)) {
      const expression = unwrap(node.expression), symbol = unalias(checker, checker.getSymbolAtLocation(
        ts.isPropertyAccessExpression(expression) ? expression.name : expression));
      if (symbol) constructors.set(symbol, []);
    }
    ts.forEachChild(node, collect);
  };
  collect(source);
  const scan = node => {
    if (ts.isTypeNode(node) || ts.isImportDeclaration(node)) return;
    if (ts.isIdentifier(node)) {
      const symbol = unalias(checker, checker.getSymbolAtLocation(node));
      if (constructors.has(symbol)) {
        let expression = node;
        if (ts.isPropertyAccessExpression(node.parent) && node.parent.name === node) expression = node.parent;
        while (expression.parent && unwrap(expression.parent) === expression) expression = expression.parent;
        if (!(ts.isNewExpression(expression.parent) && expression.parent.expression === expression))
          constructors.get(symbol).push({ start: node.getStart(source), reason: 'constructor escapes, is inspected, or its prototype may change' });
      }
    }
    ts.forEachChild(node, scan);
  };
  scan(source); return [...constructors.values()].flat();
}
export function classSnapshotFlowsV2(program, source, engine = new ClassFootprints()) {
  const escapes = constructorEscapes(program, source), normalized = exactComputedMembers(program, source),
    result = classSnapshotFlows(normalized.program, normalized.source, engine);
  if (escapes.length) { result.candidates = []; result.refused.push(...escapes); }
  return { ...result, computedEvidence: normalized.evidence, refused: [...result.refused, ...normalized.refused],
    originalSourceSha256: hash(source.text), analysisOnly: true };
}
export function observedGetterSnapshots(program, source, guardTrace) {
  const checker = program.getTypeChecker(), candidates = [], open = [];
  function jsxUses(symbol) {
    const uses = []; const scan = (node, jsx = false) => {
      if (jsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) jsx = true;
      if (jsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol) uses.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, jsx));
    }; scan(source); return uses;
  }
  function setup(node) {
    const block = node.parent.parent.parent, owner = block?.parent;
    if (!(ts.isVariableStatement(node.parent.parent) && ts.isBlock(block) && ts.isFunctionLike(owner) && owner.body === block)) return false;
    let callback = owner;
    while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
    const caller = ts.isCallExpression(callback.parent) ? unwrap(callback.parent.expression) : null;
    if (!caller) return true;
    const symbol = unalias(checker, checker.getSymbolAtLocation(ts.isPropertyAccessExpression(caller) ? caller.name : caller));
    // Any callback passed as an argument remains open; a direct function body
    // alone does not establish its tracking or untrack context.
    return !symbol && false;
  }
  const visit = node => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const expression = unwrap(node.initializer);
      if (ts.isPropertyAccessExpression(expression) && !expression.questionDotToken && setup(node)) {
        const uses = jsxUses(checker.getSymbolAtLocation(node.name));
        if (uses.length) for (const event of guardTrace ?? []) {
          const location = event.originalLocation;
          if (event.kind !== 'tracking-skipped' || location?.path !== source.fileName) continue;
          const offset = source.getPositionOfLineAndCharacter(location.line - 1, location.column - 1);
          if (offset < expression.getStart(source) || offset >= expression.end) continue;
          if (!existsSync(event.path) || hash(readFileSync(event.path)) !== event.sourceSha256) {
            open.push({ start: expression.getStart(source), reason: 'observed guard source bytes do not match' }); continue;
          }
          candidates.push({ code: 'OBSERVED_GETTER_SNAPSHOT_FLOW', severity: 'info', certification: false,
            category: 'intent-open', basis: 'executed tracking guard joined to a setup snapshot used in JSX; liveness intent undeclared',
            start: expression.getStart(source), end: expression.end, jsxUses: uses,
            premise: { path: event.path, start: event.start, end: event.end, sourceSha256: event.sourceSha256 },
            message: 'This displayed value was read once while the package skipped tracking. If it should stay live, read it inside JSX or a tracked computation; use untrack for an intentional snapshot.' });
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source); return { candidates, open };
}
export function snapshotFeedbackV2(browser, statics = {}) {
  const result = familyFeedback(browser, statics);
  if (result.excluded) return result;
  const notes = [...statics.classesV2?.candidates ?? [], ...statics.observedGetters?.candidates ?? []];
  for (const note of notes) {
    const prefix = statics.originalText.slice(0, note.start), lines = prefix.split('\n');
    result.feedback.push({ ...note, channel: 'source-candidate', category: 'intent-open',
      message: note.message ?? 'This setup read uses a package member with a tracking-sensitive source path. It is a snapshot when displayed later; read it inside JSX if it should stay live, or use untrack for an intentional snapshot.',
      location: { path: statics.originalPath, startByte: Buffer.byteLength(prefix), line: lines.length, column: lines.at(-1).length + 1 } });
  }
  result.gaps.push(...statics.classesV2?.refused ?? [], ...statics.observedGetters?.open ?? []);
  return result;
}
