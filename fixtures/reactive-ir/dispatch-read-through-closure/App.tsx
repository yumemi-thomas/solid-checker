import { createSignal } from "solid-js";

// Violations (SC1001), each confirmed by rc.17 in Chrome
// (STRICT_READ_UNTRACKED): a closure called in the body reads through
// `Function.prototype` (ADR 0517: the closure's summary carries the read).
export function CallThroughClosure() {
  const [r] = createSignal(3);
  const read = () => {
    r.call(null);
  };
  read();
  return <p>{r()}</p>;
}

export function BindThroughClosure() {
  const [r] = createSignal(3);
  const read = () => {
    r.bind(null)();
  };
  read();
  return <p>{r()}</p>;
}

// Uncertifiable: `slot ||= r` yields `slot` once it is set.
export function OrAssignThroughClosure() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const read = () => {
    (slot ||= r)();
  };
  read();
  return <p>{r()}</p>;
}

// Clean: the closure runs in an event handler.
export function CallInHandler() {
  const [r] = createSignal(3);
  const read = () => {
    r.call(null);
  };
  return <button onClick={() => read()}>{r()}</button>;
}
