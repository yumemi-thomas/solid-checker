import { createMemo, createSignal } from "solid-js";

// One proven defect for each of the four members of the `reactivity` group key
// (ADR 0428), and one for SC1003, which the key does not reach.

// strict-read-untracked (SC1001): the helper reads `count`, and the component
// body calls it untracked.
export function StrictRead() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled());
  return <div>static</div>;
}

// reactive-read-after-await (SC1002): the read after the await is untracked.
export function ReadAfterAwait() {
  const [count] = createSignal(0);
  const data = createMemo(async () => {
    await Promise.resolve();
    return count();
  });
  return <div>{String(data())}</div>;
}

// uncalled-accessor (SC1005): the accessor is interpolated uncalled.
export function Uncalled() {
  const [count] = createSignal(0);
  return <div>{`count is ${count}`}</div>;
}

// reactive-handler-frozen (SC1007): `onCustom-event` is lowered as a listener
// and is bound to a number.
export function FrozenHandler() {
  const numberHandler = 1;
  return <button onCustom-event={numberHandler}>count</button>;
}

// no-destructure (SC1003, not a member): the parameter is destructured at
// setup and its caller passes a reactive value.
function Label({ text }: { text: string }) {
  return <div title={text}>label</div>;
}

export function Caller() {
  const [count] = createSignal(0);
  return <Label text={String(count())} />;
}

// Silent in every variant: the corrected forms.
export function Corrected() {
  const [count] = createSignal(0);
  return <button onClick={() => count()}>{count()}</button>;
}
