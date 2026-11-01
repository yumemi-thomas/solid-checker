import { createEffect, createRenderEffect, createSignal, createTrackedEffect, untrack } from "solid-js";

function run(cb: () => unknown) {
  cb();
}

// A rerun calls the cleanup in the apply's window, where rc.17 warns; the
// owner's disposal calls it outside: obligations.
export function EffectRerun() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  createEffect(
    () => tick(),
    () => {
      return () => {
        count();
      };
    },
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

export function RenderRerun() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  createRenderEffect(
    () => tick(),
    () => () => count(),
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

export function RerunThroughLayer() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  createEffect(
    () => tick(),
    (value) => {
      if (value > 1) return;
      return () => {
        run(() => count());
      };
    },
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

// An apply passed by name: the same cleanup, the same obligation.
export function NamedApply() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  const apply = () => () => {
    count();
  };
  createEffect(() => tick(), apply);
  function applyDeclared() {
    return () => {
      count();
    };
  }
  createRenderEffect(() => tick(), applyDeclared);
  const applyHanded = () => () => {
    const read = () => {
      count();
    };
    [1].forEach(read);
  };
  createEffect(() => tick(), applyHanded);
  return <button onClick={() => setTick(2)}>go</button>;
}

// The read in the apply itself stays a violation.
export function ApplyRead() {
  const [count] = createSignal(3);
  createEffect(
    () => 1,
    () => {
      count();
    },
  );
  return <button>go</button>;
}

// A compute that reads nothing never reruns: only a disposal calls the
// cleanup, outside the window.
export function NeverReruns() {
  const [count] = createSignal(3);
  createEffect(
    () => 1,
    () => () => {
      count();
    },
  );
  createRenderEffect(
    () => {
      return "x";
    },
    () => () => count(),
  );
  return <button>go</button>;
}

// A tracked effect calls its cleanup inside its compute, tracking.
export function TrackedCleanup() {
  const [count] = createSignal(3);
  createTrackedEffect(() => () => {
    count();
  });
  return <button>go</button>;
}

// `untrack` closes the window.
export function UntrackedCleanup() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  createEffect(
    () => tick(),
    () => () => untrack(() => count()),
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

// A project function named like the primitive is no effect.
function createEffectLike(compute: () => number, apply: (value: number) => () => void) {
  return compute.length + apply.length;
}

export function ShadowedEffect() {
  const [count] = createSignal(3);
  const [tick] = createSignal(1);
  createEffectLike(
    () => tick(),
    () => () => {
      count();
    },
  );
  return <button>go</button>;
}
