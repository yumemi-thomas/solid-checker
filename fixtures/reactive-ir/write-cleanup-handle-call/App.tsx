import {
  action,
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  getOwner,
  onCleanup,
  runWithOwner,
  Show,
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

// --- A callback registered by name is judged like an inline one -------------

// Violation: `fn` is a `const` arrow nothing else reads, so `h()` runs it.
export function HandleByNameInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const fn = () => {
      setV(30);
    };
    const h = onCleanup(fn);
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
// Uncertifiable: a `let` may hold another function at the registration.
export function HandleByNameMutable() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    let fn = () => {
      setV(31);
    };
    const h = onCleanup(fn);
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
// Uncertifiable: the registered value is one of two functions.
export function HandleByNameChoice(props: { flag: boolean }) {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const fn = () => {
      setV(32);
    };
    const other = () => {};
    const h = onCleanup(props.flag ? fn : other);
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}

// --- Uncertifiable: the handle is called where the run is not proven --------

// A function that returns JSX and is not rendered: nothing proves it runs as
// a component. At runtime the callback runs only in the disposal.
export function HandleInUnrenderedChild() {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(40);
  });
  function Child() {
    h();
    return <span />;
  }
  return <div>{v()}</div>;
}
export function OwnerHandleInUnrenderedChild() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const c = onCleanup(() => runWithOwner(o, () => setV(41)));
  function Child() {
    c();
    return <span />;
  }
  return <div>{v()}</div>;
}
export function ActionHandleInUnrenderedChild() {
  const h = onCleanup(() => {
    act();
  });
  function Child() {
    h();
    return <span />;
  }
  return <div />;
}
// The child is handed to code that may call it as a plain function.
export function HandleInChildPassedOpaque(props: {
  slot: (child: () => unknown) => unknown;
}) {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(42);
  });
  function Child() {
    h();
    return <span />;
  }
  return (
    <div>
      {props.slot(Child) as any}
      {v()}
    </div>
  );
}
// An instance field initializer runs when an instance is constructed.
export function HandleInInstanceField() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    const h = onCleanup(() => {
      setV(43);
    });
    class K {
      f = h();
    }
    return 1;
  });
  return <div>{v()}</div>;
}
// Legal control: a child the component renders is a component, so the call
// in it is one a render makes under the component's owner.
export function HandleInRenderedChild() {
  const [v, setV] = createSignal(0);
  const h = onCleanup(() => {
    setV(44);
  });
  function Child() {
    h();
    return <span />;
  }
  return (
    <div>
      <Child />
      {v()}
    </div>
  );
}

// --- A write after an await runs on a later task, with no owner ------------

// Legal: the await precedes the write on every path, so the callback's call
// at `h()` returns before the write runs (rc.17: no throw).
export function HandleWriteAfterAwait() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    const h = onCleanup(() => {
      const f = async () => {
        await Promise.resolve();
        setV(50);
      };
      f();
    });
    h();
    return 1;
  });
  return <div>{m() + v()}</div>;
}
// Uncertifiable: inside a JSX expression the await is not proven to precede
// the write (a tracked region), so the write is not placed at `h()`. Both of
// the fuzzer's repros (an action call, and a signal write behind two awaits).
export function HandleActionAfterAwaitInJsx() {
  const [o] = createSignal(1);
  return (
    <div>
      <Show when={o()}>
        <p>
          {(() => {
            const h1 = onCleanup(() => {
              const f1 = async () => {
                await Promise.resolve();
                act();
              };
              f1();
            });
            h1();
            return 1;
          })()}
        </p>
      </Show>
    </div>
  );
}
export function HandleWriteAfterTwoAwaitsInJsx() {
  const [w, setW] = createSignal(0);
  const [o] = createSignal(1);
  return (
    <div>
      <output>{w()}</output>
      <Show when={o()}>
        <p>
          {(() => {
            const h1 = onCleanup(() => {
              const f2 = async () => {
                await Promise.resolve();
                const f1 = async () => {
                  await Promise.resolve();
                  setW(51);
                };
                f1();
              };
              f2();
            });
            h1();
            return 1;
          })()}
        </p>
      </Show>
    </div>
  );
}
