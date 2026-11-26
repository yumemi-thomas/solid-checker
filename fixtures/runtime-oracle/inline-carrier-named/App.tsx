import { createSignal, flush } from "solid-js";

// ADR 1214 (slice B): a function bound to a name whose only use is being handed whole to a standard call's inline-callback slot
// (`[1].forEach(f)`) runs on that call's stack, synchronously, as a literal written in the slot would (`layer_run`). The read is in a
// `flush` callback inside `f`: the callback takes the role of its call, which the climb now places at the carrier's call.

// Clean: the carrier runs in a tracked JSX expression, so the read tracks.
export function TrackedEach() {
  const [count] = createSignal(1);
  const f = () => {
    flush(() => {
      count();
    });
  };
  return (
    <p>
      {(() => {
        [1].forEach(f);
        return 1;
      })()}
    </p>
  );
}

// Violation: the carrier runs in the component body, so the read is untracked (STRICT_READ_UNTRACKED).
export function BodyEach() {
  const [count] = createSignal(1);
  const f = () => {
    flush(() => {
      count();
    });
  };
  [1].forEach(f);
  return <p>b</p>;
}

// Kept as before (uncertifiable): the function is also handed elsewhere, so a run of it may be anywhere.
const kept: (() => void)[] = [];
export function AlsoKept() {
  const [count] = createSignal(1);
  const f = () => {
    flush(() => {
      count();
    });
  };
  kept.push(f);
  [1].forEach(f);
  return <p>c</p>;
}

// Kept uncertifiable (the member variant): the receiver is a project object, not a standard array.
export function OwnForEach() {
  const [count] = createSignal(1);
  const own = { forEach: (cb: () => void) => queueMicrotask(cb) };
  const f = () => {
    flush(() => {
      count();
    });
  };
  own.forEach(f);
  return <p>d</p>;
}

// Kept uncertifiable (the shadowed variant): a `let` reassigned before the carrier runs it.
export function Reassigned() {
  const [count] = createSignal(1);
  let f = () => {
    flush(() => {
      count();
    });
  };
  f = () => {};
  [1].forEach(f);
  return <p>e</p>;
}
