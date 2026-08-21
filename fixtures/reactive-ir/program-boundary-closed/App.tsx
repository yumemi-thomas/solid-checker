// `.solid-checker/runtime.json` asserts `programBoundary: "closed"`: the
// analyzed files are the whole program, so an export reaches no caller outside
// them. That is evidence the analyzer cannot derive -- nothing inside a
// tsconfig proves nothing outside it imports from the tsconfig -- and it is
// the same class of user-supplied premise as `rendering`.
//
// It removes exactly one assumption: that an *additional, unseen* caller
// exists. It licenses no guessing. Every caller must still be enumerated,
// every reference must still resolve to a use the analyzer understands, and a
// missing reference list is still the absence of a fact. The paired
// open-world fixtures (engine/eslint-reactivity-*, eslint-plugin-corpus*)
// keep the default and keep their obligations.
import { createEffect, createRoot, createSignal } from "solid-js";

// Exported, and every call site passes a static value. Open-world this is an
// obligation, because the caller that passes a signal may live in another
// package. Closed, the caller set is complete: the prop compiles to a plain
// property and the read is correct.
export function StaticOnly(props: { label: string }) {
  const label = props.label;
  return <h1>{label}</h1>;
}

// Exported and passed a dynamic value. A witness proves this either way -- the
// assertion must not be what makes a violation appear.
export function Dynamic(props: { title: string }) {
  const title = props.title;
  return <h1>{title}</h1>;
}

// Exported helper whose only call site is inside createRoot. Open-world the
// owner is unprovable, because an unseen caller supplies none. Closed, the
// enumerated call site is the whole caller set and the owner is proven.
export function setupRooted() {
  const [n] = createSignal(0);
  createEffect(() => n(), (value) => sink(value));
}

// Exported helper called at module scope with no owner. A closed boundary
// makes this *more* provable, not less: it is a violation, not silence.
export function setupBare() {
  const [n] = createSignal(0);
  createEffect(() => n(), (value) => sink(value));
}

// Still an obligation under a closed boundary: the component is handed to a
// receiver as a value, and closing the program says nothing about what that
// receiver passes it.
export function PassedAsValue(props: { note: string }) {
  const note = props.note;
  return <h1>{note}</h1>;
}

export function Host() {
  const [dynamic] = createSignal("x");
  render(PassedAsValue);
  return (
    <div>
      <StaticOnly label="fixed" />
      <Dynamic title={dynamic()} />
    </div>
  );
}

createRoot(() => { setupRooted(); });
setupBare();

declare function render(component: (props: { note: string }) => unknown): void;
declare function sink(value: unknown): void;
