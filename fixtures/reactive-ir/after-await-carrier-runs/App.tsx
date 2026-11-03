import { createMemo, createSignal } from "solid-js";

// ADR 0566: SC1002 proves a read in a callback a standard-library carrier runs only where the
// carrier provably runs it (ADR 0563's `carrier_provably_runs`).

// Violations: the carrier runs the callback at least once.
export function OneElementLiteral() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    [1].forEach(() => {
      r();
    });
    return 1;
  });
  return <p>{m()}</p>;
}

export function OneElementNamed() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    [1].map(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function CallReceiver() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    hf.call(null);
    Reflect.apply(hf, null, []);
    return 1;
  });
  return <p>{m()}</p>;
}

// Uncertifiable, never a violation: the receiver may be empty, so the callback may never run.
export function EmptyLiteral() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    ([] as number[]).forEach(() => {
      r();
    });
    return 1;
  });
  return <p>{m()}</p>;
}

export function EmptyNamed() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    ([] as number[]).forEach(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function UnknownLength(props: { xs: number[] }) {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    const xs = props.xs;
    await Promise.resolve();
    xs.forEach(() => {
      r();
    });
    return 1;
  });
  return <p>{m()}</p>;
}

export function SetForEach() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    new Set<number>().forEach(() => {
      r();
    });
    return 1;
  });
  return <p>{m()}</p>;
}

export function MappedAsyncEmpty() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.all(
      ([] as number[]).map(async () => {
        await Promise.resolve();
        r();
      }),
    );
    return 1;
  });
  return <p>{m()}</p>;
}

// A project `forEach` is no carrier: the function it is handed is ADR 0565's obligation.
export function ShadowedForEach() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const list = {
      forEach(callback: () => void) {
        callback();
      },
    };
    const hf = () => {
      r();
    };
    list.forEach(hf);
    return 1;
  });
  return <p>{m()}</p>;
}
