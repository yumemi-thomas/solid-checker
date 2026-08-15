import { affects, createEffect, createMemo, createOptimistic, createOptimisticStore, createProjection, createSignal, createStore, refresh } from "solid-js";

createEffect(() => 1);
createEffect(() => 1, undefined);
// Proven non-function apply arguments crash the effect queue at runtime
// (`null.effect` / `5.effect is not a function`): flagged like the absent
// form. The `{ effect, error }` object form is legal and stays silent.
createEffect(() => 1, null);
createEffect(() => 1, 5);
createEffect(() => 1, "apply");
const applyFn = (value: number) => {};
const recoverFn = (error: unknown) => {};
createEffect(() => 1, { effect: applyFn, error: recoverFn });

createMemo(async () => 1, { sync: true });
createSignal(async () => 1, { sync: true });
createOptimistic(async () => 1, { sync: true });
// The store family never routes options.sync into its node (the runtime
// rebuilds node options with only loadingValue/name), so sync: true is inert
// here and these three stay silent.
createStore(async () => ({ value: 1 }), { value: 0 }, { sync: true });
createProjection(async () => ({ value: 1 }), { value: 0 }, { sync: true });
createOptimisticStore(async () => ({ value: 1 }), { value: 0 }, { sync: true });

createMemo(() => 1, { sync: true });
createEffect(() => 1, () => {});

const target = createMemo(() => 1);
const [signal] = createSignal(0);
const [store] = createStore({ value: 1 });
refresh(target);
// Extra refresh arguments are silently ignored by the runtime: no finding.
refresh(target, true);
refresh(() => target());
refresh({});
refresh();
// A value-form store owns no compute node: refresh throws in dev.
refresh(store);
affects(signal, "value");
affects(target, "value");
affects(store, "value", "extra");
affects(signal());

// Store child records carry the brand ($TARGET trap), so member-expression
// targets on a store base are legal affects targets — including chains
// through .at(...) on arrays.
const [state] = createStore({ user: { name: "a" }, messages: [{ status: "b" }] });
affects(state.user, "name");
affects(state.messages.at(-1)!, "status");
// ...but refresh needs a refreshable base, and this store is value-form.
refresh(state.user);

// Function-form stores and projections own a compute node: refresh is legal
// on the binding and on child records.
const [derived] = createStore(() => ({ user: { name: "a" } }), { user: { name: "" } });
const projected = createProjection(() => ({ value: 1 }), { value: 0 });
refresh(derived);
refresh(derived.user);
refresh(projected);
affects(derived.user, "name");

// A member chain rooted at an accessor reads a plain value off the accessor
// function itself: flagged, the runtime throws INVALID_REFRESH_TARGET.
refresh(target.name);

export function App() {
  refresh(target);
  return <div />;
}
