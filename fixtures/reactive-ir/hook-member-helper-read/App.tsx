import { createSignal } from "solid-js";

// ADR 1093: a hook's returned object carries a signal's getter (`useIt: count`), and a helper the JSX child reads calls it
// (`thing.useIt()`). The call is the read of the transported getter itself, not a read through a value the call hands over
// (it hands over nothing): the read is the helper's, claimed where the JSX child runs it, tracked.
export function Shown() {
  const [count] = createSignal(0);
  const useThing = () => ({ useIt: count });
  const thing = useThing();
  const shown = () => thing.useIt();
  return <p>{shown()}</p>;
}

// Kept: the same helper called in the component body runs untracked (SC1001 violation).
export function BodyHelper() {
  const [count] = createSignal(0);
  const useThing = () => ({ useIt: count });
  const thing = useThing();
  const shown = () => thing.useIt();
  const first = shown();
  return <p>{first}</p>;
}

// Kept: the member read directly in the body (SC1001 violation).
export function BodyRead() {
  const [count] = createSignal(0);
  const useThing = () => ({ useIt: count });
  const thing = useThing();
  const first = thing.useIt();
  return <p>{first}</p>;
}
