import { createSignal } from "solid-js";

// A component a parent renders by JSX alone (not exported): a function its
// body runs in place still runs while the body does (fuzzer mount axis).

function run(cb: () => unknown) {
  cb();
  return 1;
}

function never(cb: () => unknown) {
  void cb;
  return 1;
}

// Violation: an IIFE in the body writes.
function ByIife() {
  const [count, setCount] = createSignal(0);
  (() => {
    setCount(1);
  })();
  return <p>{count()}</p>;
}

// Violation: a helper runs the callback that writes, during the call.
function ByHelper() {
  const [count, setCount] = createSignal(0);
  run(() => {
    setCount(2);
  });
  return <p>{count()}</p>;
}

// Violation: a closure the body calls writes (reached before this change too).
function ByClosure() {
  const [count, setCount] = createSignal(0);
  const step = () => {
    setCount(3);
  };
  step();
  return <p>{count()}</p>;
}

// Uncertifiable: a helper that never calls its callback; the write never runs,
// and no proof places it.
function ByNeverHelper() {
  const [count, setCount] = createSignal(0);
  never(() => {
    setCount(4);
  });
  return <p>{count()}</p>;
}

export function Page() {
  return (
    <div>
      <ByIife />
      <ByHelper />
      <ByClosure />
      <ByNeverHelper />
    </div>
  );
}
