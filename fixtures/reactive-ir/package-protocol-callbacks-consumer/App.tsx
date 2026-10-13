import { createSignal } from "solid-js";
import { access, accessAlways, accessOpen, compare } from "reactive-package";
import * as primitives from "reactive-package";

// The claim under test. `access`'s accepted contract closes `callbacks` over a
// call of argument 0 and a `get` of it (`v.length`); `compare`'s closes it over
// a `coerce` of each argument (`a < b`). A property read or a coercion of the
// caller's value runs that value's own code, at the call, in the caller's
// tracking context, so neither is an invocation the consumer models and
// neither leaves anything open at the import.
export function Counter() {
  const [count] = createSignal(0);
  const initial = access(count);
  const fixed = access(3);
  const order = compare(count, 3);
  return (
    <div>
      {initial}
      {fixed}
      {order}
      {access(count)}
      {compare(count, 3)}
    </div>
  );
}

// The control. The same declaration and the same use, with `callbacks` left
// open: the package may run the accessor at any time, so the import reports the
// open claim. The closures above have to remove exactly this finding.
export function CounterOpen() {
  const [count] = createSignal(0);
  return <div>{accessOpen(count)}</div>;
}

// Only the call protocol reads the accessor. An event handler runs outside
// the component body's strict-read window, and a static argument is no read.
export function Handler() {
  const [count] = createSignal(0);
  return <div onClick={() => access(count)}>{access(3)}</div>;
}

// A spread does not prove the exact runtime argument at the contract's slot.
export function Spread() {
  const [count] = createSignal(0);
  const initial = access(...([count] as const));
  return <div>{initial}</div>;
}

// A namespace binding this fixture receipt does not admit must fail closed.
export function NamespaceRead() {
  const [count] = createSignal(0);
  const initial = primitives.access(count);
  return <div>{initial}</div>;
}

// A transparent TypeScript wrapper does not read count.
export function WrapperRead() {
  const [count] = createSignal(0);
  const initial = access(count satisfies () => number);
  return <div>{initial}</div>;
}

// A same-named local function that only stores its argument is no invocation
// of the package export and introduces no accessor read.
export function Shadowed() {
  const [count] = createSignal(0);
  const access = (read: () => number) => [read];
  const readers = access(count);
  return <div>{readers.length}</div>;
}

// The cardinality control: unlike access, this fixture's accepted export
// guarantees its ambient inline invocation on every call.
export function GuaranteedRead() {
  const [count] = createSignal(0);
  const initial = accessAlways(count);
  return <div>{initial}{accessAlways(count)}</div>;
}
