// Reuse small source premises for more native rule concepts. These projections
// remain source assumptions; a surrogate violation is never promoted to proof
// about the original package. Exact copied spans and binding evidence are required.
import { resolve } from 'node:path';
import { projectWarning, ts } from './lower.mjs';
const callbackRules = new Set(['reactive-write-in-owned-scope', 'resolve-in-tracked-scope',
  'until-in-tracked-scope', 'reactive-read-after-await']);
const accessorRules = new Set(['uncalled-accessor', 'prefer-for']);
export function projectFamilyWarning(finding, lowered, original, modeledPath, program) {
  const existing = projectWarning(finding, lowered, original, modeledPath); if (existing) return existing;
  if (finding.kind !== 'violation' || resolve(finding.primaryLocation.path) !== resolve(modeledPath)) return null;
  function map(location, end = false) {
    if (!location || resolve(location.path) !== resolve(modeledPath)) return null;
    const offset = Buffer.from(lowered.text).subarray(0, end ? location.endByte : location.startByte).toString('utf8').length;
    const segment = lowered.segments.find(s => s.start <= offset && (end ? offset <= s.end : offset < s.end));
    return segment ? segment.originalStart + (segment.copied ? offset - segment.start : 0) : null;
  }
  const start = map(finding.primaryLocation); if (start === null) return null;
  const related = [...finding.relatedLocations ?? [], ...finding.evidence?.map(e => e.location) ?? []].map(location => map(location)).filter(p => p !== null);
  const end = map(finding.primaryLocation, true), calls = new Map(), identifiers = [], exact = [];
  function visit(node) {
    if (ts.isCallExpression(node)) calls.set(node.getStart(original), node);
    if (ts.isIdentifier(node)) identifiers.push(node);
    if (node.getStart(original) === start && node.end === end) exact.push(node);
    ts.forEachChild(node, visit);
  }
  visit(original);
  function unwrap(node) {
    while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node) ||
      ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
    return node;
  }
  // The native rule proves the operation. Resolve only its exact operand back
  // to the modeled declaration; no contained-symbol or name-based matching.
  const operands = exact.flatMap(node => {
    if (finding.rule === 'uncalled-accessor' && ts.isIdentifier(node)) return [node];
    if (finding.rule !== 'prefer-for' || !ts.isCallExpression(node) || !ts.isPropertyAccessExpression(node.expression)) return [];
    const receiver = unwrap(node.expression.expression);
    const callee = receiver && ts.isCallExpression(receiver) ? unwrap(receiver.expression) : null;
    return callee && ts.isIdentifier(callee) ? [callee] : [];
  });
  const checker = program?.getTypeChecker(), symbols = new Set(operands.map(node => checker?.getSymbolAtLocation(node)).filter(Boolean));
  const admitted = lowered.sites.filter(site => {
    if (!site.applied || site.behavior?.inert) return false;
    if (accessorRules.has(finding.rule)) return site.behavior?.returns && site.bindings.some(binding => {
      if (related.some(p => binding.start <= p && p < binding.end)) return true;
      const declaration = identifiers.find(node => node.getStart(original) === binding.start && node.end === binding.end);
      return declaration && symbols.has(checker?.getSymbolAtLocation(declaration));
    });
    if (callbackRules.has(finding.rule) && site.behavior?.trackedCallback !== undefined) {
      const argument = calls.get(site.start)?.arguments[site.behavior.trackedCallback];
      return argument && (ts.isArrowFunction(argument) || ts.isFunctionExpression(argument)) &&
        argument.body.getStart(original) <= start && start < argument.body.end;
    }
    return false;
  });
  // A primary span must identify exactly one applied premise.
  if (admitted.length !== 1) return null;
  const site = admitted[0], position = original.getLineAndCharacterOfPosition(start);
  return { id: finding.id, rule: finding.rule, severity: 'warning', basis: site.basis ?? 'source-extracted-assumption',
    certification: false, category: finding.rule === 'prefer-for' ? 'preference' : 'source-assumption',
    package: site.package, export: site.export, modelVersion: site.modelVersion,
    message: `Under the source model for ${site.package}.${site.export}: ${finding.message}`,
    location: { path: original.fileName, startByte: Buffer.byteLength(original.text.slice(0, start)), line: position.line + 1, column: position.character + 1 },
    analyzerFindingKind: finding.kind, projectedPremise: accessorRules.has(finding.rule) ? 'returned-accessor' : 'tracked-callback' };
}

export function projectFamilyWarnings(findings, lowered, original, modeledPath, program) {
  const warnings = findings.map(f => projectFamilyWarning(f, lowered, original, modeledPath, program)).filter(Boolean);
  const leafLocations = new Set(warnings.filter(w => w.rule === 'leaf-owner-forbidden-call').map(w => JSON.stringify(w.location)));
  // A leaf owner is present, but it forbids cleanup registration. A surrogate
  // cannot also assert that this exact operation has no owner.
  return warnings.filter(w => w.rule !== 'missing-owner' || !leafLocations.has(JSON.stringify(w.location)));
}
