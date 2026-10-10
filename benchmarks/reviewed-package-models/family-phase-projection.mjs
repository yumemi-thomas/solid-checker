// Restore provenance for inline callbacks copied into generated memo text.
// The old lowerer anchors the whole generated operation at the package call.
import { ts } from './lower.mjs';
import { projectFamilyWarnings } from './family-project-warning.mjs';
function unwrap(node) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node) ||
    ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  return node;
}
export function callbackSourceMap(lowered, original, modeledProgram, modeledPath) {
  const modeled = modeledProgram.getSourceFile(modeledPath), checker = modeledProgram.getTypeChecker();
  const target = symbol => symbol && symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
  const memoSymbols = new Set();
  for (const statement of modeled.statements) if (ts.isImportDeclaration(statement) && statement.moduleSpecifier.text === 'solid-js') {
    const module = checker.getSymbolAtLocation(statement.moduleSpecifier);
    for (const symbol of module ? checker.getExportsOfModule(module) : []) if (symbol.name === 'createMemo') memoSymbols.add(target(symbol));
  }
  const originalCalls = new Map(), nativeCalls = [];
  function originalVisit(node) { if (ts.isCallExpression(node)) originalCalls.set(node.getStart(original), node); ts.forEachChild(node, originalVisit); }
  function modelVisit(node) {
    if (ts.isCallExpression(node) && memoSymbols.has(target(checker.getSymbolAtLocation(unwrap(node.expression))))) nativeCalls.push(node);
    ts.forEachChild(node, modelVisit);
  }
  originalVisit(original); modelVisit(modeled);
  const mappings = [], gaps = [];
  for (const site of lowered.sites.filter(site => site.applied && site.behavior?.trackedCallback !== undefined)) {
    const argument = originalCalls.get(site.start)?.arguments[site.behavior.trackedCallback];
    const originalCallback = argument && unwrap(argument);
    if (!originalCallback || !ts.isArrowFunction(originalCallback) && !ts.isFunctionExpression(originalCallback)) continue;
    const candidates = nativeCalls.flatMap(call => {
      const clone = call.arguments[0] && unwrap(call.arguments[0]);
      if (!clone || !ts.isArrowFunction(clone) && !ts.isFunctionExpression(clone)) return [];
      const segment = lowered.segments.find(segment => !segment.copied && segment.originalStart === site.start &&
        segment.start <= call.getStart(modeled) && clone.end <= segment.end);
      return segment && clone.getText(modeled) === originalCallback.getText(original) ? [{ segment, clone }] : [];
    });
    if (candidates.length !== 1) { gaps.push({ start: site.start, reason: 'callback clone has no unique native-symbol provenance' }); continue; }
    const { segment, clone } = candidates[0];
    mappings.push({ segment, start: clone.getStart(modeled), end: clone.end, originalStart: originalCallback.getStart(original),
      siteStart: site.start, native: 'solid-js.createMemo', package: site.package, export: site.export });
  }
  const segments = lowered.segments.flatMap(segment => {
    const matches = mappings.filter(mapping => mapping.segment === segment).sort((a, b) => a.start - b.start);
    if (!matches.length) return [segment];
    if (matches.some((mapping, index) => index > 0 && matches[index - 1].end > mapping.start)) return [segment];
    const parts = []; let cursor = segment.start;
    for (const mapping of matches) {
      if (cursor < mapping.start) parts.push({ ...segment, start: cursor, end: mapping.start });
      parts.push({ start: mapping.start, end: mapping.end, originalStart: mapping.originalStart, copied: true }); cursor = mapping.end;
    }
    if (cursor < segment.end) parts.push({ ...segment, start: cursor });
    return parts;
  });
  return { lowered: { ...lowered, segments }, evidence: mappings.map(({ segment, ...mapping }) => mapping), gaps };
}
export function phaseWarnings(findings, lowered, original, originalProgram, modeledProgram, modeledPath) {
  const mapped = callbackSourceMap(lowered, original, modeledProgram, modeledPath);
  return { warnings: projectFamilyWarnings(findings, mapped.lowered, original, modeledPath, originalProgram), evidence: mapped.evidence, gaps: mapped.gaps };
}
