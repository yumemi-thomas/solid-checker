// Positive callback-context assumptions from bounded installed-source paths.
// No callback cardinality, full execution coverage or certification is implied.
import { SourceExtractor, literal, objectValue, unknownValue } from './source-extractor.mjs';
import { ts } from './lower.mjs';
export function callbackArguments(expressions) {
  const callbacks = [];
  function value(node, path) {
    if (ts.isParenthesizedExpression(node)) return value(node.expression, path);
    if (ts.isArrowFunction(node) && !ts.isBlock(node.body)) {
      const id = callbacks.length; callbacks.push({ id, path });
      return { kind: 'callback-probe', id, returnValue: value(node.body, [...path, 'return']) };
    }
    if (ts.isObjectLiteralExpression(node)) {
      const fields = {};
      for (const property of node.properties) {
        if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) return unknownValue('opaque object');
        fields[property.name.text] = value(property.initializer, [...path, property.name.text]);
      }
      return objectValue(fields);
    }
    if (ts.isArrayLiteralExpression(node)) return { kind: 'array', items: node.elements.map((item, index) => value(item, [...path, index])) };
    if (ts.isNumericLiteral(node)) return literal(Number(node.text));
    if (ts.isStringLiteral(node)) return literal(node.text);
    if ([ts.SyntaxKind.TrueKeyword, ts.SyntaxKind.FalseKeyword].includes(node.kind)) return literal(node.kind === ts.SyntaxKind.TrueKeyword);
    if (node.kind === ts.SyntaxKind.NullKeyword) return literal(null);
    if (ts.isIdentifier(node) && node.text === 'undefined') return literal(undefined);
    return unknownValue('consumer input is not a closed value');
  }
  return { values: expressions.map((node, index) => value(node, [index])), callbacks };
}
export class CallbackPaths extends SourceExtractor {
  constructor(host, options) { super(host, options); this.footprints = []; }
  merge(states, outer) {
    super.merge(states, outer);
    const required = states[0]?.operations.filter(op => op.kind === 'callback' && states.every(s => s.operations.some(other =>
      other.kind === 'callback' && other.id === op.id && other.context === op.context))) ?? [];
    outer.operations = [...new Map([...outer.operations, ...required].map(op => [JSON.stringify(op), op])).values()];
  }
  invoke(value, args, state, depth) {
    if (depth > this.maxDepth) return super.invoke(value, args, state, depth);
    if (value.kind === 'callback-probe') {
      const operation = { kind: 'callback', id: value.id, context: state.context ?? 'caller-unknown',
        path: state.callModule.path, start: state.callNode.getStart(state.callModule.source) };
      state.operations.push(operation); this.footprints.push(operation); return value.returnValue;
    }
    if (value.kind === 'function' && (value.node.asteriskToken || value.node.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword))) {
      state.blockers.push('async/generator helper timing is open'); return unknownValue('deferred helper');
    }
    if (value.kind === 'native') {
      if (value.name === 'getObserver') return state.context === 'tracked-compute' ? literal(true) :
        ['untracked', 'effect-apply', 'settled-apply'].includes(state.context) ? literal(null) : unknownValue('caller observer');
      if (value.name === 'getOwner' && !state.owned) return unknownValue('caller owner');
      if (['createMemo', 'createEffect', 'createRenderEffect', 'createTrackedEffect', 'onSettled'].includes(value.name)) {
        const visit = (callback, context) => {
          if (!callback) return;
          const inner = { ...state, env: new Map(state.env), operations: [...state.operations], unknowns: [...state.unknowns], blockers: [...state.blockers], owned: true, context };
          this.invoke(callback, [], inner, depth + 1); this.merge([inner], state);
        };
        visit(args[0], value.name === 'onSettled' ? 'settled-apply' : 'tracked-compute');
        if (['createEffect', 'createRenderEffect'].includes(value.name)) visit(args[1]?.kind === 'object' ? args[1].fields.effect : args[1], 'effect-apply');
        // The base knows the result/owner operation; it does not evaluate these
        // callback bodies, so the context visit above is not executed twice.
        return super.invoke(value, args, state, depth);
      }
      if (value.name === 'untrack') {
        const inner = { ...state, env: new Map(state.env), operations: [...state.operations], unknowns: [...state.unknowns], blockers: [...state.blockers], context: 'untracked' };
        const result = this.invoke(args[0] ?? unknownValue('missing callback'), [], inner, depth + 1); this.merge([inner], state); return result;
      }
      if (value.name === 'createRoot') {
        const inner = { ...state, env: new Map(state.env), operations: [...state.operations], unknowns: [...state.unknowns], blockers: [...state.blockers], owned: true, context: 'root-setup' };
        const result = this.invoke(args[0] ?? unknownValue('missing callback'), [], inner, depth + 1); this.merge([inner], state); return result;
      }
    }
    return super.invoke(value, args, state, depth);
  }
  profile(path, name, args) {
    this.steps = 0; this.footprints = [];
    const module = this.module(path), state = { env: new Map(), owned: false, operations: [], unknowns: [], blockers: [], context: 'caller-unknown' };
    const target = this.exportValue(module, name, state, 0);
    if (target.kind !== 'function') return { assumptions: [], footprints: [], gaps: ['export is not a resolved function'] };
    this.invoke(target, args, state, 0);
    const assumptions = state.blockers.length ? [] : state.operations.filter(op => op.kind === 'callback');
    return { assumptions: [...new Map(assumptions.map(op => [JSON.stringify(op), op])).values()],
      footprints: [...new Map(this.footprints.map(op => [JSON.stringify(op), op])).values()], gaps: [...new Set([...state.blockers, ...state.unknowns])], steps: this.steps };
  }
}
