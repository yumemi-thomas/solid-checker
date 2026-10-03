// Bounded, package-independent consumer input synthesis. Types admit a witness;
// they do not establish the runtime meaning of the witness or a package premise.
import { ts } from './lower.mjs';
export const seeds = ["document.createElement('div')", 'new EventTarget()', 'document', 'window',
  'new AbortController().signal', 'new Date(0)', 'setInterval', 'setTimeout', 'requestAnimationFrame'];
export function synthesize(checker, type, location, seedTypes, profile = 0, depth = 0, seen = new Set()) {
  if (depth > 6 || seen.has(type)) return null;
  const next = new Set(seen); next.add(type);
  const recur = target => synthesize(checker, target, location, seedTypes, profile, depth + 1, next);
  const flags = type.flags;
  if (flags & ts.TypeFlags.Never) return null;
  const opaque = profile === 2 ? '() => 1' : '1';
  if (flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown)) return opaque;
  if (flags & ts.TypeFlags.TypeParameter) { const constraint = checker.getBaseConstraintOfType(type); return constraint ? recur(constraint) : opaque; }
  if (flags & ts.TypeFlags.StringLiteral) return JSON.stringify(type.value);
  if (flags & ts.TypeFlags.NumberLiteral) return String(type.value);
  if (flags & ts.TypeFlags.BooleanLiteral) return type.intrinsicName;
  if (flags & ts.TypeFlags.String) return JSON.stringify(profile === 2 ? '*' : 'test');
  if (flags & ts.TypeFlags.Number) return profile === 2 ? '10' : '1';
  if (flags & ts.TypeFlags.Boolean) return profile === 2 ? 'false' : 'true';
  if (flags & (ts.TypeFlags.Void | ts.TypeFlags.Undefined)) return 'undefined';
  if (flags & ts.TypeFlags.Null) return 'null';
  if (type.isUnion()) {
    const branches = type.types.filter(t => !(t.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Null)));
    for (let offset = 0; offset < branches.length; offset++) {
      const value = recur(branches[(profile + offset) % branches.length]); if (value !== null) return value;
    }
    return type.types.some(t => t.flags & ts.TypeFlags.Undefined) ? 'undefined' : null;
  }
  // Assignability is checked against the actual lib DOM/runtime declaration,
  // rather than recognizing a package parameter or type by its spelling.
  const signatures = type.getCallSignatures();
  if (signatures.length) {
    for (let i = 0; i < seedTypes.length; i++) if (seedTypes[i] === type) return seeds[i];
    const result = recur(checker.getReturnTypeOfSignature(signatures[0]));
    return result === null ? null : `() => (${result})`;
  }
  for (let i = 0; i < seedTypes.length; i++) if (checker.isTypeAssignableTo(seedTypes[i], type)) return seeds[i];
  if (checker.isTupleType(type)) {
    const values = checker.getTypeArguments(type).map(recur); return values.every(v => v !== null) ? `[${values.join(', ')}]` : null;
  }
  if (checker.isArrayType(type) || checker.isArrayLikeType(type)) {
    if (profile === 0) return '[]';
    const item = checker.getIndexTypeOfType(type, ts.IndexKind.Number), value = item && recur(item);
    return value == null ? '[]' : `[${value}]`;
  }
  const allProps = checker.getPropertiesOfType(type), required = allProps.filter(p => !(p.flags & ts.SymbolFlags.Optional));
  const props = [...required, ...(profile === 1 ? allProps.filter(p => p.flags & ts.SymbolFlags.Optional).slice(0, 4) : [])];
  if (props.length > 8 || type.getConstructSignatures().length || checker.getIndexInfosOfType(type).length) return null;
  const fields = [];
  for (const prop of props) {
    if (prop.getName().startsWith('__@') || prop.declarations?.some(d => d.modifiers?.some(m => [ts.SyntaxKind.PrivateKeyword, ts.SyntaxKind.ProtectedKeyword].includes(m.kind)))) return null;
    const value = recur(checker.getTypeOfSymbolAtLocation(prop, location)); if (value === null) return null;
    fields.push(`${JSON.stringify(prop.getName())}: ${value}`);
  }
  return flags & ts.TypeFlags.Object ? `{${fields.join(', ')}}` : null;
}
