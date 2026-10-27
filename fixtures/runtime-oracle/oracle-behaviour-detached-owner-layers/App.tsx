import {
  createEffect,
  createMemo,
  createOwner,
  createSignal,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

declare global {
  interface Window {
    landedIife: number;
    landedForEach: number;
    landedUntrack: number;
    landedMemo: number;
    landedEffect: number;
    landedNested: number;
    landedDirect: number;
    landedUntrackNull: number;
  }
}
window.landedIife = 0;
window.landedForEach = 0;
window.landedUntrack = 0;
window.landedMemo = 0;
window.landedEffect = 0;
window.landedNested = 0;
window.landedDirect = 0;
window.landedUntrackNull = 0;

// `runWithOwner(null, fn)` runs `fn` with no owner: the write lands, whatever
// function it sits in (ADR 0455). Each counter is bumped after the write, so it
// is 1 only when the write did not throw.
export function NullIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    (() => {
      setV(1);
      window.landedIife++;
    })();
  });
  return <p>{v()}</p>;
}
export function NullForEach() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    [1].forEach(() => {
      setV(1);
      window.landedForEach++;
    });
  });
  return <p>{v()}</p>;
}
export function NullUntrackIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => {
      (() => {
        setV(1);
        window.landedUntrack++;
      })();
    });
  });
  return <p>{v()}</p>;
}
export function NullIifeInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runWithOwner(null, () => {
      (() => {
        setV(1);
        window.landedMemo++;
      })();
    });
    return 1;
  });
  return <p>{v()}{m()}</p>;
}
export function NullIifeInEffect() {
  const [v, setV] = createSignal(0);
  createEffect(
    () => {
      runWithOwner(null, () => {
        (() => {
          setV(1);
          window.landedEffect++;
        })();
      });
      return 1;
    },
    () => {},
  );
  return <p>{v()}</p>;
}
export function NullNested() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    (() => {
      [1].forEach(() => {
        (() => {
          setV(1);
          window.landedNested++;
        })();
      });
    })();
  });
  return <p>{v()}</p>;
}
export function NullDirect() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    setV(1);
    window.landedDirect++;
  });
  return <p>{v()}</p>;
}

// Controls: a real owner reaches the write and rc.17 throws
// (`REACTIVE_WRITE_IN_OWNED_SCOPE`); the dev diagnostic is covered by the
// finding. A bound `createOwner()` result is not proven non-null: uncertifiable.
export function CreatedOwnerIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    (() => {
      setV(1);
    })();
  });
  return <p>{v()}</p>;
}
export function ComponentOwnerIife() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  runWithOwner(owner, () => {
    (() => {
      setV(1);
    })();
  });
  return <p>{v()}</p>;
}
export function BoundOwnerIife() {
  const [v, setV] = createSignal(0);
  const owner = createOwner();
  runWithOwner(owner, () => {
    (() => {
      setV(1);
    })();
  });
  return <p>{v()}</p>;
}

// Controls, continued: the same owners reach a write directly in the callback,
// and `untrack` keeps the component's owner for a function it runs in place.
export function ComponentOwnerDirect() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  runWithOwner(owner, () => {
    setV(1);
  });
  return <p>{v()}</p>;
}
export function CreatedOwnerDirect() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    setV(1);
  });
  return <p>{v()}</p>;
}
export function BoundOwnerDirect() {
  const [v, setV] = createSignal(0);
  const owner = createOwner();
  runWithOwner(owner, () => {
    setV(1);
  });
  return <p>{v()}</p>;
}
export function UntrackIife() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    (() => {
      setV(1);
    })();
  });
  return <p>{v()}</p>;
}
export function UntrackForEachInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    untrack(() => {
      [1].forEach(() => {
        setV(1);
      });
    });
    return 1;
  });
  return <p>{v()}{m()}</p>;
}

// A closure bound to a name inside an `untrack` callback runs where it is
// called (ADR 0455): under a detached owner the write lands; in the callback's
// own pass under the component's owner rc.17 throws, so the lexical deferred
// role of the callback was no proof. Handed by name to `forEach`, or to a
// helper, the callback runs in the pass too.
export function UntrackClosureUnderNullOwner() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => {
      const bump = () => {
        setV(1);
        window.landedUntrackNull++;
      };
      bump();
    });
  });
  return <p>{v()}</p>;
}
export function UntrackClosureCalled() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => setV(1);
    bump();
  });
  return <p>{v()}</p>;
}
export function UntrackClosureViaMap() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => {
      return setV(1);
    };
    [1].map(() => {
      return bump();
    });
  });
  return <p>{v()}</p>;
}
export function UntrackFunctionInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    untrack(() => {
      function bump() {
        setV(1);
      }
      bump();
    });
    return 1;
  });
  return <p>{v()}{m()}</p>;
}
export function UntrackClosureByName() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => setV(1);
    [1].forEach(bump);
  });
  return <p>{v()}</p>;
}
function callNow(callback: () => void) {
  callback();
}
export function UntrackLiteralToHelper() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    callNow(() => setV(1));
  });
  return <p>{v()}</p>;
}
