import { createMemo, createReaction, createSignal, onSettled } from "solid-js";

// ADR 1203: rc.21's `onSettled` returns without throwing (it runs its callback on no path before it returns), so a call of it
// between a closure's declaration and its run in a `createReaction` effect does not stop the effect first (ADR 1166's interruption
// test). The run executes the read in the effect's untracked pass, where rc.21 warns STRICT_READ_UNTRACKED.

// Violation: the closure is handed to `onSettled`, then run through a carried `forEach`.
export function SettledCarried() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(f);
      [f].forEach((g) => g());
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Violation: handed to `onSettled`, then called by name.
export function SettledCalled() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(f);
      f();
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Violation: a function literal handed to `onSettled` before the run.
export function SettledLiteral() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(() => {});
      [f].forEach((g) => g());
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Not a violation: `createMemo(f)` in the effect throws PRIMITIVE_IN_EFFECT_CALLBACK before the run (rc.21), so the read never runs.
export function MemoBefore() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const track1 = createReaction(() => {
    (() => {
      const f = () => r1();
      createMemo(f);
      f();
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Not a violation: the argument is a property read, which may throw before `onSettled` is called (a getter).
export function SettledMember() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const box: { readonly cb: () => void } = {
    get cb(): () => void {
      throw new Error("no");
    },
  };
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(box.cb);
      [f].forEach((g) => g());
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Not a violation: a local `onSettled` that throws, which shadows the import.
export function ShadowedSettled() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const onSettled = (_cb: () => void): void => {
    throw new Error("no");
  };
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(f);
      [f].forEach((g) => g());
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}
