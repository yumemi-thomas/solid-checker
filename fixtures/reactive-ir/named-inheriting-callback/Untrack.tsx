import { createSignal, untrack } from "solid-js";

// Violation (SC2001, rc.17 in Chrome: REACTIVE_WRITE_IN_OWNED_SCOPE): a write
// in a closure a callback named at two `untrack` positions in the body runs.
export function UntrackShared() {
  const [, setW] = createSignal(0);
  const run = () => {
    const write = () => { setW(7); };
    write();
  };
  untrack(run);
  untrack(run);
  return <div></div>;
}

// Uncertifiable: both positions in handlers, which no rejection decides.
export function UntrackSharedInHandlers() {
  const [, setW] = createSignal(0);
  const run = () => { const write = () => { setW(7); }; write(); };
  return <div><button onClick={() => untrack(run)}>a</button><button onClick={() => untrack(run)}>b</button></div>;
}

// Clean: the callback is a click handler.
export function HandlerByName() {
  const [, setW] = createSignal(0);
  const run = () => { const write = () => { setW(7); }; write(); };
  return <div><button onClick={run}>x</button></div>;
}
