import {
  action,
  createEffect,
  createMemo,
  createOwner,
  createRenderEffect,
  createRoot,
  createSignal,
  createStore,
  createTrackedEffect,
  flush,
  getOwner,
  onCleanup,
  refresh,
  runWithOwner,
  Show,
  untrack,
  type Owner,
} from "solid-js";

// The write guard stands down while the runtime disposes an owner
// (@solidjs/signals@2.0.0-rc.17 `dist/dev-shared.js:5620-5626`, `disposalDepth`,
// raised by `runDisposal` at `:1314`), so a write that an `onCleanup` callback
// runs under a proven owner lands without throwing. The same call outside a
// cleanup throws (ADR 0455 amendment).

// --- Legal: the callback runs only in a disposal ----------------------------

// A component's own owner, captured in the body.
export function CleanupComponentOwner() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => setV(1)));
  return <div>{v()}</div>;
}
// An owner the call creates.
export function CleanupCreatedOwner() {
  const [v, setV] = createSignal(0);
  onCleanup(() => runWithOwner(createOwner(), () => setV(2)));
  return <div>{v()}</div>;
}
// Through an inline standard-library callback.
export function CleanupOwnerForEach() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => [1].forEach(() => setV(3))));
  return <div>{v()}</div>;
}
// A root the cleanup creates: its body runs during the cleanup.
export function CleanupCreateRoot() {
  const [v, setV] = createSignal(0);
  onCleanup(() => createRoot(() => setV(4)));
  return <div>{v()}</div>;
}

// --- Uncertifiable: a named function a cleanup may call --------------------

// The function runs where it is called. A cleanup hands it on or calls it, but
// nothing here reads its other callers: neither a violation nor legal.
export function CleanupNamedOwnerHelper() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const f = () => runWithOwner(o, () => setV(10));
  onCleanup(f);
  return <div>{v()}</div>;
}
export function CleanupCallsOwnerHelper() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const f = () => runWithOwner(o, () => setV(11));
  onCleanup(() => f());
  return <div>{v()}</div>;
}
export function CleanupCallsRootHelper() {
  const [v, setV] = createSignal(0);
  const g = () => createRoot(() => setV(12));
  onCleanup(() => g());
  return <div>{v()}</div>;
}

// --- Uncertifiable: an effect's returned cleanup, no run proven outside a disposal --

// An effect apply's returned cleanup is no registration: a rerun that finds the
// owner alive calls it outside `runDisposal`, and the write throws. But the
// owner's disposal (a memo that reruns, a `Show` turned off, a removed row, a
// disposed root) calls it inside one, where the write lands, and nothing here
// orders the owner's disposal against the rerun: the effect below tracks `n`,
// which a button changes, and may rerun with its owner alive, yet no run is
// proven outside a disposal.
export function EffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => n(),
    () => () => runWithOwner(o, () => setV(6))
  );
  return <button onClick={() => setN(n() + 1)}>{v()}</button>;
}
export function RenderEffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  createRenderEffect(
    () => n(),
    () => () => runWithOwner(o, () => setV(40))
  );
  return <button onClick={() => setN(n() + 1)}>{v()}</button>;
}
export function TrackedEffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  createTrackedEffect(() => {
    n();
    return () => runWithOwner(o, () => setV(41));
  });
  return <button onClick={() => setN(n() + 1)}>{v()}</button>;
}
// Fuzzer shapes (Chrome-confirmed legal on rc.17): the host is disposed before
// the effect reruns, so the cleanup runs only in a disposal.

// A memo host that tracks what its effect tracks: a bump disposes the effect
// first.
export function MemoHostTracksSame() {
  const [w, setW] = createSignal(0);
  const [t, setT] = createSignal(0);
  createMemo(() => {
    t();
    createEffect(
      () => {
        t();
        return 1;
      },
      () => () => {
        runWithOwner(createOwner(), () => {
          setW(17);
        });
      }
    );
    return 1;
  });
  return <button onClick={() => setT(1)}>{w()}</button>;
}
// The same under a root, with a store setter in try/finally.
export function MemoHostStoreSetterInRoot() {
  const [w, setW] = createStore({ v: 0 });
  const [t, setT] = createSignal(0);
  createMemo(() => {
    t();
    createEffect(
      () => {
        t();
        return 1;
      },
      () => () => {
        createRoot(() => {
          try {
          } finally {
            setW((s) => {
              s.v = 17;
            });
          }
        });
      }
    );
    return 1;
  });
  return <button onClick={() => setT(1)}>{w.v}</button>;
}
// A `Show` host turned off disposes the effect and runs the cleanup.
export function ShowHostNestedOwner() {
  const [w, setW] = createSignal(0);
  const [t, setT] = createSignal(0);
  const [on, setOn] = createSignal(1);
  return (
    <div>
      <Show when={on()}>
        {(() => {
          createEffect(
            () => {
              t();
              return 1;
            },
            () => () => {
              runWithOwner(createOwner(), () => {
                setW(17);
              });
            }
          );
          return 1;
        })()}
      </Show>
      <button onClick={() => setOn(0)}>{w()}</button>
      <button onClick={() => setT(1)}>go</button>
    </div>
  );
}
// A `Show` host, a refresh in a closure handed to `createRoot` by name.
export function ShowHostRefreshInRoot() {
  const m = createMemo(() => 1);
  const [t, setT] = createSignal(0);
  const [on, setOn] = createSignal(1);
  return (
    <div>
      <Show when={on()}>
        {(() => {
          createEffect(
            () => {
              t();
              return 1;
            },
            () => () => {
              const f = () => {
                refresh(m);
              };
              createRoot(f);
            }
          );
          return 1;
        })()}
      </Show>
      <button onClick={() => setOn(0)}>go</button>
      <button onClick={() => setT(1)}>go</button>
    </div>
  );
}
// --- Violations: the same owner outside a disposal --------------------------

// An effect apply that runs on a flush outside every cleanup runs outside a
// disposal.
export function EffectApplyUnderOwner() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => 0,
    () => {
      runWithOwner(o, () => setV(42));
    }
  );
  return <div>{v()}</div>;
}
// A named function the body hands to `createRoot` runs in the body.
export function NamedRootBodyInComponent() {
  const [v, setV] = createSignal(0);
  function init() {
    setV(43);
  }
  createRoot(init);
  return <div>{v()}</div>;
}
// A cleanup that schedules the write runs it after the disposal is over.
export function CleanupSchedulesWrite() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    setTimeout(() => runWithOwner(o, () => setV(7)));
  });
  return <div>{v()}</div>;
}
// A cleanup registered, but the write is written in the body.
export function CleanupRegisteredWriteInBody() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {});
  runWithOwner(o, () => setV(8));
  return <div>{v()}</div>;
}
// A cleanup that only builds the function: it runs where it is called. The
// memo reruns when `n` changes, its cleanup then defines `later`, and a click
// calls it with the disposal over.
export function CleanupStoresFunction() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  let later: (() => void) | undefined;
  const m = createMemo(() => {
    n();
    onCleanup(() => {
      later = () => runWithOwner(o, () => setV(9));
    });
    return v();
  });
  return (
    <div>
      <button onClick={() => setN(n() + 1)}>{m()}</button>
      <button onClick={() => later?.()} />
    </div>
  );
}
// An action call's guard does not stand down in a disposal.
const act = action(function* () {});
export function CleanupActionUnderOwner() {
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => act()));
  return <div />;
}

// --- Violations: an await ahead of the write ends the disposal -------------

// The async cleanup suspended, so the owner is entered after `runDisposal`
// returned: the guard is back.
export function AsyncCleanupAwaitOwner() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(async () => {
    await Promise.resolve();
    runWithOwner(o, () => setV(20));
  });
  return <div>{v()}</div>;
}
export function AsyncCleanupAwaitRoot() {
  const [v, setV] = createSignal(0);
  onCleanup(async () => {
    await null;
    createRoot(() => setV(21));
  });
  return <div>{v()}</div>;
}
export function AsyncCleanupAwaitCreatedOwner() {
  const [v, setV] = createSignal(0);
  onCleanup(async () => {
    await 0;
    runWithOwner(createOwner(), () => setV(22));
  });
  return <div>{v()}</div>;
}
export function AsyncForEachInCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    [1].forEach(async () => {
      await 0;
      runWithOwner(o, () => setV(23));
    });
  });
  return <div>{v()}</div>;
}
export function AsyncIifeInCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    (async () => {
      await 0;
      runWithOwner(o, () => setV(24));
    })();
  });
  return <div>{v()}</div>;
}
// Fresh stacks: the disposal is over when they run.
export function CleanupThen() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    Promise.resolve().then(() => runWithOwner(o, () => setV(25)));
  });
  return <div>{v()}</div>;
}
export function CleanupMicrotask() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    queueMicrotask(() => runWithOwner(o, () => setV(26)));
  });
  return <div>{v()}</div>;
}

// --- Violations: the handle runs the callback outside the disposal --------

// rc.17's `onCleanup` returns the callback it registered, so calling the handle
// in the body runs the write there, under the owner, with no disposal around
// it (ADR 0456).
export function CleanupHandleCalled() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const c = onCleanup(() => runWithOwner(o, () => setV(30)));
  c();
  return <div>{v()}</div>;
}
export function CleanupHandleCalledAtOnce() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => setV(31)))();
  return <div>{v()}</div>;
}

// --- Uncertifiable: a function a cleanup may run, no call site read -------

function register(fn: () => void) {
  onCleanup(fn);
}
export function HelperRegistersCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  register(() => runWithOwner(o, () => setV(32)));
  return <div>{v()}</div>;
}
export function AssignedThenCleanupCalls() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  let later: () => void = () => {};
  later = () => runWithOwner(o, () => setV(33));
  onCleanup(() => later());
  return <div>{v()}</div>;
}
export function InlineHelperParameter() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const run = (fn: () => void) => fn();
  onCleanup(() => run(() => runWithOwner(o, () => setV(34))));
  return <div>{v()}</div>;
}
export function ObjectMethodCalledByCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const api = {
    stop() {
      runWithOwner(o, () => setV(35));
    },
  };
  onCleanup(() => api.stop());
  return <div>{v()}</div>;
}
const [shared, setShared] = createSignal(0);
function writeUnder(o: Owner | null) {
  runWithOwner(o, () => setShared(36));
}
export function DeclaredHelperFromCleanup() {
  const o = getOwner();
  onCleanup(() => writeUnder(o));
  return <div>{shared()}</div>;
}

// --- Legal: a function passed by name to a root the cleanup creates --------

// `createRoot(init)` runs `init` as the root body, here inside the cleanup.
export function CleanupNamedRootBody() {
  const [v, setV] = createSignal(0);
  function init() {
    setV(44);
  }
  onCleanup(() => createRoot(init));
  return <div>{v()}</div>;
}
export function CleanupConstRootBody() {
  const [v, setV] = createSignal(0);
  const init = () => setV(45);
  onCleanup(() => createRoot(init));
  return <div>{v()}</div>;
}

// --- Uncertifiable: an effect that may never rerun ---------------------------

// A rerun calls the returned cleanup outside a disposal, but an effect that
// tracks nothing it can see change never reruns: the cleanup runs once, in the
// final teardown (inside a disposal, where rc.17 lets the write land).
export function EffectCleanupConstantCompute() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => 0,
    () => () => runWithOwner(o, () => setV(46))
  );
  return <div>{v()}</div>;
}
export function RenderEffectCleanupConstantCompute() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createRenderEffect(
    () => 0,
    () => () => runWithOwner(o, () => setV(47))
  );
  return <div>{v()}</div>;
}
export function TrackedEffectCleanupNoReads() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createTrackedEffect(() => () => runWithOwner(o, () => setV(48)));
  return <div>{v()}</div>;
}
// The only write of the tracked signal is the cleanup's own: the cleanup runs
// only after a rerun, which only the cleanup can cause.
export function EffectCleanupTracksItsOwnTarget() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => v(),
    () => () => runWithOwner(o, () => setV(49))
  );
  return <div>{v()}</div>;
}
// An untracked read subscribes to nothing.
export function EffectCleanupUntrackedRead() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => untrack(n),
    () => () => runWithOwner(o, () => setV(50))
  );
  return <button onClick={() => setN(n() + 1)}>{v()}</button>;
}
// A signal nothing writes never changes.
export function EffectCleanupReadsConstantSignal() {
  const [v, setV] = createSignal(0);
  const [n] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => n(),
    () => () => runWithOwner(o, () => setV(51))
  );
  return <div>{v()}</div>;
}

// --- Uncertifiable: an effect created and flushed in a cleanup -------------

// The apply runs inside the disposal.
export function CleanupFlushesEffect() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    createEffect(
      () => 0,
      () => {
        runWithOwner(o, () => setV(52));
      }
    );
    flush();
  });
  return <div>{v()}</div>;
}

// --- Legal: a root the cleanup creates, with an inline layer in it ---------

export function CleanupRootWithMap() {
  const [v, setV] = createSignal(0);
  onCleanup(() =>
    createRoot(() => {
      [1, 2].map(() => setV(37));
    })
  );
  return <div>{v()}</div>;
}
// A non-arrow function expression is the same callback.
export function CleanupFunctionExpression() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(function () {
    runWithOwner(o, () => setV(38));
  });
  return <div>{v()}</div>;
}
