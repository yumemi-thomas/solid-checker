import { createSignal } from "solid-js";
import { access, accessOpen, compare } from "reactive-package";

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
