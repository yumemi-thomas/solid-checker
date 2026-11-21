import { createReaction, createSignal } from "solid-js";

// ADR 1176 (N8): a read in a layer that runs in place inside a `createReaction` effect (a local closure it calls, an IIFE)
// runs in that effect's untracked pass, where rc.21 warns STRICT_READ_UNTRACKED.

// Violation: a closure the effect calls.
export function ClosureInReaction() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    const f1 = () => {
      r1();
    };
    f1();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Violation: an IIFE in the effect.
export function IifeInReaction() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    (() => {
      r1();
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Clean: the closure runs in the tracking function, which tracks.
export function ClosureInTracking() {
  const [r1] = createSignal(3);
  const track1 = createReaction(() => {});
  track1(() => {
    const f1 = () => {
      r1();
    };
    f1();
  });
  return <div />;
}
