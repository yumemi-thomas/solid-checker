// A separate seed-parameterized sampler keeps historical inputs reproducible.
// Native seed assignability is checked against real declarations; no package
// parameter names or assertion casts select a value.
import { ts } from './lower.mjs';
import { seeds as originalSeeds } from './argument-witnesses.mjs';
export const standardSeeds = [...originalSeeds, 'new Uint8Array([1, 2])', 'new ArrayBuffer(2)',
  'new DataView(new ArrayBuffer(2))', 'new Blob([new Uint8Array([1, 2])])'];

export function extendedWitness(checker, type, location, seedTypes, profile = 1, depth = 0, seen = new Set()) {
  if (depth > 6 || seen.has(type)) return null;
  const next = new Set(seen); next.add(type);
  const recur = target => extendedWitness(checker, target, location, seedTypes, profile, depth + 1, next);
  const flags = type.flags, opaque = profile === 2 ? '() => 1' : '1';
  if (flags & ts.TypeFlags.Never) return null;
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
  const signatures = type.getCallSignatures();
  if (signatures.length) {
    for (let i = 0; i < seedTypes.length; i++) if (seedTypes[i] === type) return standardSeeds[i];
    const signature = signatures[0], resultType = checker.getReturnTypeOfSignature(signature), first = signature.getParameters()[0];
    const byteSeed = standardSeeds.indexOf('new Uint8Array([1, 2])');
    if (first && signature.minArgumentCount >= 1 && checker.getTypeOfSymbolAtLocation(first, location).flags & ts.TypeFlags.Number &&
      resultType.flags & ts.TypeFlags.Object && checker.isTypeAssignableTo(seedTypes[byteSeed], resultType))
      return 'arg0 => new Uint8Array(arg0)';
    const result = recur(resultType); return result === null ? null : `() => (${result})`;
  }
  for (let i = 0; i < seedTypes.length; i++) if (checker.isTypeAssignableTo(seedTypes[i], type)) return standardSeeds[i];
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
