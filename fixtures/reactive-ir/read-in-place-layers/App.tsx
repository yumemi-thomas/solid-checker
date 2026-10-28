import * as S from "solid-js";
import {
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

function callIt(cb: () => void) {
  cb();
}

let kept: (() => void) | undefined;

function callAndKeep(cb: () => void) {
  cb();
  kept = cb;
}

function handBack(cb: () => void) {
  return cb;
}

// Violations: the layer runs on the stack of its call, in the created root's
// body or the effect apply, whose strict-read window is open (rc.17 warns
// STRICT_READ_UNTRACKED in each, fast path).

export function RootDirect() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    r();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootIife() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    (() => {
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootForEach() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    [1].forEach(() => {
      r();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootHelper() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    callIt(() => {
      r();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootHelperAlsoKept() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    callAndKeep(() => {
      r();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootNamespaceIife() {
  const [r, setR] = S.createSignal(1);
  S.createRoot(() => {
    (() => {
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootAsyncIifeBeforeAwait() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    (async () => {
      r();
      await Promise.resolve();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function OwnerIife() {
  const [r, setR] = createSignal(1);
  runWithOwner(getOwner(), () => {
    (() => {
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyIife() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      (() => {
        r();
      })();
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyForEach() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      [1].forEach(() => {
        r();
      });
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyHelper() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      callIt(() => {
        r();
      });
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyNestedLayers() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      (() => {
        [1].forEach(() => {
          callIt(() => {
            r();
          });
        });
      })();
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

// The equivalence pair: `setK(r())` in the apply is reported; wrapped in an
// IIFE it is the same read on the same stack.
export function ApplySetterOfReadIife() {
  const [r, setR] = createSignal(1);
  const [k, setK] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      (() => {
        setK(r());
      })();
    }
  );
  return <button onClick={() => setR(k() + 2)}>x</button>;
}

// Legal (rc.17 silent): the layer's read is in a nearer tracked computation,
// an `untrack`, a fresh stack, past an await, a function returned and called
// from a handler, a helper that never calls its callback, a layer in JSX
// (the insert's computation tracks it), or a root that is a local function.

export function RootMemoIife() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    createMemo(() => {
      (() => {
        r();
      })();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootUntrackIife() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    untrack(() => {
      (() => {
        r();
      })();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyUntrackForEach() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      untrack(() => {
        [1].forEach(() => {
          r();
        });
      });
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

export function ApplyTimer() {
  const [r, setR] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      setTimeout(() => {
        r();
      }, 0);
    }
  );
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootAsyncIifeAfterAwait() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    (async () => {
      await Promise.resolve();
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootReturnedLayer() {
  const [r] = createSignal(1);
  const run = createRoot(() => {
    const read = () => {
      r();
    };
    return read;
  });
  return <button onClick={() => run()}>x</button>;
}

export function RootHelperNeverCalls() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    handBack(() => {
      r();
    });
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootJsxIife() {
  const [r, setR] = createSignal(1);
  const view = createRoot(() => <div>{(() => r())()}</div>);
  return (
    <div>
      {view}
      <button onClick={() => setR(2)}>x</button>
    </div>
  );
}

export function ShadowedRoot() {
  const [r, setR] = createSignal(1);
  const createRoot = (fn: () => void) => {
    kept = fn;
  };
  createRoot(() => {
    (() => {
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

// Uncertifiable: rc.17 warns here, but no proof places the read in the
// window: a default value of the layer, which runs only when its call omits
// the argument (ADR 0454), and a layer whose await only may precede the read.

export function RootIifeDefault() {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    ((x = r()) => x)();
  });
  return <button onClick={() => setR(2)}>x</button>;
}

export function RootIifeMaybeAwait(props: { wait: boolean }) {
  const [r, setR] = createSignal(1);
  createRoot(() => {
    (async () => {
      if (props.wait) await Promise.resolve();
      r();
    })();
  });
  return <button onClick={() => setR(2)}>x</button>;
}
