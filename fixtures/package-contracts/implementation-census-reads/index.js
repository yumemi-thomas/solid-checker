// The **clean** half of the `reads` implementation census tracer: whether one invocation of
// an export observes the current value of a reactive source **it owns**.
//
// `semantic-model.md` § reads [Decision 2026-09-10] puts a read reached
// through a caller-supplied value on the *caller's* side of the line, so these
// exports split on the receiver's **provenance**, not on syntax: every one of
// them is a property access or an invocation, and what differs is whose value
// is underneath.
//
// No export imports `solid-js`. The census cannot prove a receiver is not a
// proxy, so a real Solid store would add nothing this fixture does not already
// state, and would make every row depend on an accepted-dependency closure the
// way `implementation-census-creates`' README warns about.

// A value this module built whose members the specification created with
// CreateDataPropertyOrThrow (ADR 0044). Reading one runs no code at all.
const ownLiteral = { limit: 10, label: "fixture" };

// --- `reads: []` must close for every export below. ---

// No access of any kind.
export function plainArithmetic(a, b) {
  return a + b;
}

// An own object literal: data properties, so no read (ADR 0044).
export function readsOwnLiteral() {
  return ownLiteral.limit;
}

// A property access whose receiver the caller supplied. Whatever getter sits
// on it is code the caller attached (ADR 0034), so the read is the caller's.
export function readsCallerMember(props) {
  return props.value;
}

// An element access on the same caller-supplied receiver (ADR 0034's other
// admitted form).
export function readsCallerElement(props, key) {
  return props[key];
}

// The export's act is the *invocation*, which is a `callbacks` item; whatever
// the caller's accessor reads is read by the caller's code (census plan § 3.2).
export function invokesCallerAccessor(read) {
  return read();
}

// --- `reads: []` must close for every export above. ---
//
// Nothing in this module installs a property accessor at run time, so its
// closure carries no `runtime-accessor-installation` hazard and the census is
// allowed to decide the domain. The proxy lives in `./owned`, a separate
// entrypoint with a separate closure, because the hazard is a fact about the
// closure and not about one export: a single `new Proxy` anywhere in this file
// would withdraw `reads` from every export in it.
