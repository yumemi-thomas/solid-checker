import { createEffect, createMemo, createSignal } from "solid-js";
import * as helpers from "./helpers";

// ADR 0565: a function written after the await and run through a project hand-off.

// A same-file wrapper that calls its parameter on every completion of its own synchronous body.
function runNow(g: () => void): void {
  g();
}

export function Wrapper() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    const run = (g: () => void) => {
      g();
    };
    run(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function ModuleWrapper() {
  const [r] = createSignal(3);
  createEffect(
    async () => {
      await Promise.resolve();
      function hf() {
        r();
      }
      runNow(hf);
      return 1;
    },
    () => {},
  );
  return <p />;
}

// Uncertifiable: the wrapper may not run it here, or runs it where the walk does not follow.
export function WrapperConditional() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    const run = (g: () => void, on: boolean) => {
      if (on) g();
    };
    run(hf, false);
    return 1;
  });
  return <p>{m()}</p>;
}

export function WrapperDefers() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    const run = (g: () => void) => {
      setTimeout(g, 0);
    };
    run(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

let saved: (() => void) | undefined;
export function WrapperStores() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    // Shadows the module's `runNow`: this one keeps the callback.
    const runNow = (g: () => void) => {
      saved = g;
    };
    runNow(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function Namespace() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    helpers.run(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function Member() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    const ho = { m: hf };
    ho.m();
    return 1;
  });
  return <p>{m()}</p>;
}

export function Assigned() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const ho: { m?: () => void } = {};
    ho.m = () => {
      r();
    };
    ho.m?.();
    return 1;
  });
  return <p>{m()}</p>;
}

export function AssignedThis() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    class Holder {
      m?: () => void;
      constructor() {
        this.m = () => {
          r();
        };
      }
    }
    new Holder().m?.();
    return 1;
  });
  return <p>{m()}</p>;
}

export function LiteralHandOff() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const keep = (g: () => void) => {
      saved = g;
    };
    keep(() => {
      r();
    });
    saved?.();
    return 1;
  });
  return <p>{m()}</p>;
}

// Written before the await, handed on after it.
export function HandedAfterAwait() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    const hf = () => {
      r();
    };
    await Promise.resolve();
    runNow(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

// Violation (ADR 0351, unchanged): a closed object literal's member names its function.
export function SlotBeforeAwait() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    const ho = {
      m: () => {
        r();
      },
    };
    await Promise.resolve();
    ho.m();
    return 1;
  });
  return <p>{m()}</p>;
}

export function AssignedBeforeAwait() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    const ho: { m?: () => void } = {};
    ho.m = () => {
      r();
    };
    await Promise.resolve();
    ho.m?.();
    return 1;
  });
  return <p>{m()}</p>;
}

// Clean: deferred, never run, run before the await, or a source already tracked.
export function Timer() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    setTimeout(hf, 0);
    return 1;
  });
  return <p>{m()}</p>;
}

export function NeverCalled() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    return 1;
  });
  return <p>{m()}</p>;
}

export function BeforeAwait() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    const hf = () => {
      r();
    };
    runNow(hf);
    await Promise.resolve();
    return 1;
  });
  return <p>{m()}</p>;
}

export function Tracked() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    r();
    await Promise.resolve();
    const hf = () => {
      r();
    };
    runNow(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

export function Returned() {
  const [r] = createSignal(3);
  const m = createMemo(async () => {
    await Promise.resolve();
    const hf = () => {
      r();
    };
    return hf;
  });
  return <p>{typeof m()}</p>;
}
