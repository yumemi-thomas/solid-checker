// Why the generator's own `creates` walk declined to propose, one export per
// blocker kind.
//
// This is the *measurement* fixture for `CreatesProposalWalk`, not another
// census fixture: nothing here is certified, and what it pins is the
// `declinedClosures` array of the proposal refusal sidecar — the record that
// says which blocker stopped a `creates: []` proposal from being made at all.
// See README.md for what each export is for and, importantly, for what
// `dialect-silent` does and does not mean here. The export that still
// *proposes* is `./clean`, in its own module: this one's top-level
// `import "solid-js"` is a closure hazard that opens every domain of this
// artifact case regardless of the walk, so a control here would prove nothing.
import { createEffect } from "solid-js";

// `createEffect` is canonical Solid 2.0 vocabulary and no dialect's audited
// negative authority carries a `creates` denial row for that spelling
// (`rust/crates/solid-dialect/src/solid_2.rs`: the row was *withdrawn*
// 2026-09-04). Silence is "do not propose", so this declines with
// `dialect-silent`, naming the resolved package and the canonical spelling —
// which is exactly the pair `scripts/dialect-audit-yield.mjs` ranks.
export function dialectSilent(value) {
  createEffect(
    () => value,
    () => {}
  );
}

// Byte-identical body, one call away. The walk is lexical, so nothing inside
// `viaSilentHelper` refuses on its own; the fixpoint over the resolved local
// call edge is what refuses the call into this helper. The export therefore
// declines twice: `refusing-callee-fixpoint`, naming the helper's exact
// declaration span, *and* the helper's own `dialect-silent` record, kept at
// its own location inside the helper. Reporting only the first would make the
// primitive invisible in the ranking on exactly the shape real consumer
// packages have.
function silentHelper(value) {
  createEffect(
    () => value,
    () => {}
  );
}

export function viaSilentHelper(value) {
  silentHelper(value);
}

// An identifier nothing declares, which this build resolves to no symbol at
// all. "Unresolved" is never evidence of harmlessness, so the walk declines —
// and the record carries the call's location, no callee identity, because
// there is none to carry, and the callee's observed *shape*:
// `undeclared-identifier`, spelled `externalGlobal`. Deliberately a bare global
// rather than an import from an unaudited dependency: that would be a closure
// hazard decided elsewhere, and no walk decision at all — and, as it happens,
// not an unresolved callee either, because an unresolvable import still gives
// its local binding an alias symbol (see `UnresolvedCalleeShape`).
export function unresolvedCallee(value) {
  return externalGlobal(value);
}

// The rest of this module is one export per remaining `unresolved-callee`
// *shape*. Each is a call this build resolves to no symbol; what differs is
// what the callee expression is, which is the whole point — half of every
// decline the ecosystem corpus measures is `unresolved-callee`, and one kind
// name could not tell a resolver gap from a callee no analysis of the module
// could decide. The shapes' decision order is fixed and documented on
// `solid_reactive_ir::UnresolvedCalleeShape`; these exports pin it.

// The receiver resolves (a module-local `const`) and the property does not:
// `member-property-unresolved`, spelled `publish`. The record names the
// property because that is the declaration that is missing.
const registry = {};

export function memberPropertyUnresolved(value) {
  return registry.publish(value);
}

// The receiver itself resolves to nothing, so the property was never reachable:
// `member-receiver-unresolved`, still spelled with the property (`method`) —
// the receiver has no name the record could carry, and what was called is worth
// more than nothing.
export function memberReceiverUnresolved(value) {
  return externalGlobal.method(value);
}

// A computed property: there is no static property spelling at all, which is
// the shape's whole content. `computed-member` therefore carries the *receiver*
// (`handlers`), because that is the half that is nameable. Checked before
// `parameter-rooted` even though `handlers` is a parameter: no property name
// exists to report either way.
export function computedMember(handlers, key) {
  handlers[key]();
}

// The chain roots at a parameter, so the callee is whatever this module's
// caller passed and no analysis of these bytes can decide it: the census's own
// `parameter-rooted`, spelled with the leaf property (`read`).
export function parameterRooted(source) {
  return source.read();
}

// The same, one binding-initializer alias away. `roots_in_caller_parameter`
// follows up to four such hops, so this is `parameter-rooted` too rather than
// `member-receiver-unresolved` — the callee is still the caller's value.
export function parameterAliasRooted(source) {
  const alias = source;
  return alias.read();
}

// The callee is an immediately-invoked function expression: `expression-callee`,
// spelled `function-expression`. (A higher-order `factory()()` is *not* one of
// these on this build -- TypeScript answers an entity at the inner call, so the
// callee resolves and the walk never records a shape for it.)
export function expressionCallee(value) {
  return (function () {
    return value;
  })();
}

// Nothing else names this callee's syntax, so it is recorded as `other`
// carrying that syntactic kind — `await-expression` — rather than folded into a
// neighbouring shape. The catch-all exists precisely so a shape the classifier
// does not model stays visible in the ranking.
export async function otherSyntax(promised) {
  return (await promised)();
}
