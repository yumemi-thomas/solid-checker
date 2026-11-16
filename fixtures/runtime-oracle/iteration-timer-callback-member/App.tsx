import { createMemo, createSignal, createStore } from "solid-js";

// ADR 1014: a timer handed a member of a `const` object literal nothing writes, exports or hands on calls the arrow written there, which has no `this`: the
// window reaches no code, a primitive's array pattern stays exact, and the writes below are the defects they look like. Chrome rc.21 reports both; the checker
// proves both (it withdrew the slots before ADR 1014).
const bundle = { cb: () => {} };
setTimeout(bundle.cb, 0);

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}

// STORE_WRITE_OUTSIDE_SETTER: the fuzz grid's lost shape (`mx-store-write-outside-setter-timer-…-cb-bundle`): the write runs from a timer handed the member.
export function TimerWrite() {
  const [s1] = createStore({ v: 0 });
  const cb1 = {
    cb: () => {
      s1.v = 7;
    },
  };
  setTimeout(cb1.cb, 0);
  return <div>{s1.v}</div>;
}

// The fixture `dispatch-and-copies` holds `setTimeout(box.run, 0)` beside a memo with these two setter calls through `Function.prototype` (ADR 0516): writes in
// a pure computation, proved again now that the timer's member no longer withdraws the slots. One component each: the first write halts its memo, so two calls
// in one memo show Chrome only the first.
export function DispatchCallInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    setV.call(null, 1);
    return 0;
  });
  return (
    <p>
      {v()}
      {m()}
    </p>
  );
}

export function DispatchBindInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    setV.bind(null)(2);
    return 0;
  });
  return (
    <p>
      {v()}
      {m()}
    </p>
  );
}
