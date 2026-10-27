import {
  action,
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  getOwner,
  onCleanup,
  runWithOwner,
} from "solid-js";

// rc.17's `onCleanup(fn)` returns `fn` (@solidjs/signals@2.0.0-rc.17
// `dist/dev.js:2377-2405`, `dist/dev-shared.js:1415-1421`), so calling the
// handle runs the callback there and then, under the call's owner and outside
// any disposal (ADR 0456).

// --- Violations: the handle is called where the write throws -----------------

export function HandleInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const h = onCleanup(() => {
      setV(1);
    });
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
export function HandleInEffectCompute() {
  const [v, setV] = createSignal(0);
  createEffect(
    () => {
      const h = onCleanup(() => {
        setV(2);
      });
      h();
      return 1;
    },
    () => {}
  );
  return <div>{v()}</div>;
}
export function HandleInRoot() {
  const [v, setV] = createSignal(0);
  createRoot((dispose) => {
    const h = onCleanup(() => {
      setV(3);
    });
    h();
    return dispose;
  });
  return <div>{v()}</div>;
}
// The call written on the result.
export function HandleCalledAtOnce() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    onCleanup(() => {
      setV(4);
    })();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
export function HandleInBody() {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(5);
  });
  h();
  return <div>{v()}</div>;
}
// An action call's guard throws under the memo too.
const act = action(function* () {});
export function ActionHandleInMemo() {
  const m = createMemo(() => {
    const h = onCleanup(() => {
      act();
    });
    h();
    return 1;
  });
  return <div>{m()}</div>;
}
// A closure the callback calls runs with it.
export function HandleNamedClosure() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const h = onCleanup(() => {
      const f = () => {
        setV(6);
      };
      f();
    });
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
// A write under an owner the callback gives it: the handle's call is outside
// the disposal, in the body or in a handler, so the guard throws.
export function OwnerHandleInBody() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const c = onCleanup(() => runWithOwner(o, () => setV(7)));
  c();
  return <div>{v()}</div>;
}
export function OwnerHandleInHandler() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const c = onCleanup(() => runWithOwner(o, () => setV(8)));
  return <button onClick={() => c()}>{v()}</button>;
}

// --- Legal: only a disposal or a legal call runs the callback ----------------

export function HandleNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const h = onCleanup(() => {
      setV(10);
    });
    return 1;
  });
  return <div>{m() + v()}</div>;
}
export function OwnerHandleNeverCalled() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const c = onCleanup(() => runWithOwner(o, () => setV(11)));
  return <div>{v()}</div>;
}
// Another cleanup calls the handle, inside the same disposal.
export function HandleInOtherCleanup() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const h = onCleanup(() => {
      setV(12);
    });
    onCleanup(() => h());
    return 1;
  });
  return <div>{m() + v()}</div>;
}
export function HandleInHandler() {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(13);
  });
  return <button onClick={() => h()}>{v()}</button>;
}
export function HandleInBodyTimeout() {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(14);
  });
  setTimeout(() => h());
  return <div>{v()}</div>;
}

// --- Uncertifiable: the handle reaches code this does not read --------------

let saved: (() => void) | undefined;
export function HandleStored() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    saved = onCleanup(() => {
      setV(20);
    });
    return 1;
  });
  return <div>{v()}</div>;
}
function run(fn: () => void) {
  fn();
}
export function HandlePassed() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    run(
      onCleanup(() => {
        setV(21);
      })
    );
    return 1;
  });
  return <div>{v()}</div>;
}
export function HandleReturned() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() =>
    onCleanup(() => {
      setV(22);
    })
  );
  return <button onClick={() => m()()}>{v()}</button>;
}
export function HandleAliased() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    const h = onCleanup(() => {
      setV(23);
    });
    const g = h;
    g();
    return 1;
  });
  return <div>{v()}</div>;
}
// A member the callback builds: its calls are not this census's.
export function HandleObjectMember() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    const h = onCleanup(() => {
      const api = {
        f: () => {
          setV(24);
        },
      };
      api.f();
    });
    h();
    return 1;
  });
  return <div>{v()}</div>;
}
