// The unresolved exits of the static-API target rules (SC9003). The sibling
// fixture static-api pins the arity and wrapper diagnostics (SC7003) and the
// resolved happy paths; this one pins the case where the target *is* an
// identifier but the checker cannot trace it back to a branded Solid source.
import { affects, createMemo, createSignal, refresh } from "solid-js";

// refresh-target-unresolved (SC9003): `plain` resolves to an ordinary object
// binding, not a source created by createSignal/createMemo/createStore, so
// the brand refresh() needs at runtime cannot be proven.
const plain = { value: 1 };
refresh(plain);

// affects-target-unresolved (SC9003): same premise, other primitive.
affects(plain);

// The corrected forms pass the branded bindings themselves and must resolve
// silently (their reads/writes are pinned by the static-api fixture).
const doubled = createMemo(() => 1);
refresh(doubled);
const [count] = createSignal(0);
affects(count);

// A local parameter has a symbol, but no provenance tying it to a Solid
// source. These exercise the non-local SC9003 branch for both API names.
export function invalidateUnknown(target: unknown) {
  refresh(target);
  affects(target);
}

// Member-expression targets resolve through their chain root. When that root
// is not a proven Solid source — a plain object, or a store record received
// as a parameter — the call stays unresolved (SC9003), never proven-invalid:
// the base might carry the store brand at runtime.
refresh(plain.value);
export function annotate(state: { user: { name: string } }) {
  affects(state.user, "name");
}

export function App() {
  return <div>{doubled()}{count()}</div>;
}
