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
  }
}
window.landedIife = 0;
window.landedForEach = 0;
window.landedUntrack = 0;
window.landedMemo = 0;
window.landedEffect = 0;
window.landedNested = 0;
window.landedDirect = 0;

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
