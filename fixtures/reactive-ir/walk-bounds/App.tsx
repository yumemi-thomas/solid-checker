import { createEffect, createMemo, createRoot, createSignal, createTrackedEffect, onCleanup } from "solid-js";

const [c] = createSignal(1);
const [w] = createSignal(0);
const pending = createMemo(async () => 1);

const fwd0 = (g: () => void) => {
  g();
};
const fwd1 = (g: () => void) => {
  fwd0(g);
};
const reads0 = () => {
  pending();
};
const reads1 = () => {
  reads0();
};
const reads2 = () => {
  reads1();
};
const reads3 = () => {
  reads2();
};
const reads4 = () => {
  reads3();
};
const reads5 = () => {
  reads4();
};
const later0 = async () => {
  await Promise.resolve();
  c();
  return 1;
};
const later1 = () => later0();

export function Forwarded() {
  createEffect(
    () => w(),
    () => {
      fwd1(() => {
        c();
      });
    },
  );
  return <p />;
}

export function Deep() {
  createTrackedEffect(() => {
    reads5();
  });
  return <p />;
}

export function AfterAwait() {
  const m = createMemo(() => later1());
  return <p>{m()}</p>;
}

export function CleanupNoHandle() {
  createRoot(() => {
    onCleanup(() => {
      c();
    });
  });
  return <p />;
}

export function CleanupHandle() {
  const dispose = createRoot((d: () => void) => {
    onCleanup(() => {
      c();
    });
    return d;
  });
  dispose();
  return <p />;
}

// ADR 0695: a read past three helpers after an await in a loop is followed to
// its end, and is uncertifiable: the loop may not run, so it is no proved read.
// It stopped silently before.
const deep0 = () => c();
const deep1 = () => {
  deep0();
};
const deep2 = () => {
  deep1();
};
const deep3 = () => {
  deep2();
};
const deep4 = () => {
  deep3();
};

export function LongChainInLoop() {
  const m = createMemo(async () => {
    for (const item of [1]) {
      await Promise.resolve(item);
      deep4();
    }
    return 1;
  });
  return <p>{m()}</p>;
}
