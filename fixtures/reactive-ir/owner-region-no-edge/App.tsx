import { createEffect, createRoot, createSignal, onCleanup } from "solid-js";

// ADR 0534: an owner operation in a callback written inside a component body
// (or a `createRoot` callback) is answered by that region's owner only when
// the callback is proven to run in the region's synchronous extent. A callback
// no owner edge reaches, or only a possible-unowned one, may run from a timer
// or a handler instead; rc.17 warns NO_OWNER_CLEANUP / NO_OWNER_EFFECT there.
// Each component below was run on rc.17 (saturation probes e1 e2 e3 e7 e8 e9
// e10 e11 e12 e14 a3 f18 s37; controls e5 e6 e13 e15 f26).

// Module-level wrapper that hands its callback to a scheduler (e11).
function laterAtModule(cb: () => void) {
  setTimeout(cb, 0);
}

// Module-level store, called from a handler (e2).
let stored: (() => void) | undefined;
function keep(cb: () => void) {
  stored = cb;
}

// --- Unplaced (uncertifiable): the callback's run in the region is unproven.

// e1: a body-local wrapper hands the closure to `setTimeout`.
export function WrapperScheduler() {
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  later(() => {
    onCleanup(() => {});
  });
  return <p />;
}

// e11: the wrapper is at module level.
export function ModuleWrapperScheduler() {
  laterAtModule(() => {
    onCleanup(() => {});
  });
  return <p />;
}

// e12: the same under a `createRoot` callback.
export function WrapperSchedulerInRoot() {
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  createRoot(() => {
    later(() => {
      onCleanup(() => {});
    });
  });
  return <p />;
}

// e14: the wrapper is called from a handler.
export function WrapperSchedulerInHandler() {
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  return (
    <button
      onClick={() =>
        later(() => {
          onCleanup(() => {});
        })
      }
    />
  );
}

// e7: the wrapper hands it to `queueMicrotask`.
export function WrapperMicrotask() {
  function later(cb: () => void) {
    queueMicrotask(cb);
  }
  later(() => {
    onCleanup(() => {});
  });
  return <p />;
}

// e9: the operation is an effect.
export function WrapperSchedulerEffect() {
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  later(() => {
    createEffect(
      () => 1,
      () => {},
    );
  });
  return <p />;
}

// e2: the closure is stored by a module-level function and called from a handler.
export function StoredThenHandler() {
  keep(() => {
    onCleanup(() => {});
  });
  return <button onClick={() => stored?.()} />;
}

// e3: a class constructor run from a timer.
export function ClassConstructorTimer() {
  class K {
    constructor() {
      onCleanup(() => {});
    }
  }
  setTimeout(() => new K(), 0);
  return <p />;
}

// e8: a class method run from a timer.
export function ClassMethodTimer() {
  class K {
    m() {
      onCleanup(() => {});
    }
  }
  const k = new K();
  setTimeout(() => k.m(), 0);
  return <p />;
}

// e10: a closure assigned to a member and called from a timer.
export function StoredMemberTimer() {
  const box: { f?: () => void } = {};
  box.f = () => {
    onCleanup(() => {});
  };
  setTimeout(() => box.f!(), 0);
  return <p />;
}

// a3: a closure assigned to a parameter's member inside a helper the timer runs.
export function AssignedMemberInTimerHelper() {
  function make(o: { m?: () => void }) {
    o.m = () => {
      onCleanup(() => {});
    };
    o.m!();
  }
  setTimeout(() => make({}), 0);
  return <p />;
}

// f18: a named closure handed to `forEach` inside a timer.
export function ForEachNamedInTimer() {
  const f = () => {
    onCleanup(() => {});
  };
  setTimeout(() => [1].forEach(f), 0);
  return <p />;
}

// s37: an object getter read from a timer.
export function ObjectGetterTimer() {
  const o = {
    get v() {
      onCleanup(() => {});
      return 1;
    },
  };
  setTimeout(() => {
    const x = o.v;
    void x;
  }, 0);
  return <p />;
}

// e13: a closure stored in an object literal and never called. SiteReach
// cannot prove an object member unreached, so this is an obligation too: the
// recorded precision trade.
export function StoredNeverCalled() {
  const o = {
    f: () => {
      onCleanup(() => {});
    },
  };
  void o;
  return <p />;
}

// --- Controls.

// e6: a wrapper that calls its callback in its own body, run from a timer:
// the ADR 0474 census places the closure in the timer, a violation.
export function WrapperSyncInTimer() {
  function now(cb: () => void) {
    cb();
  }
  setTimeout(
    () =>
      now(() => {
        onCleanup(() => {});
      }),
    0,
  );
  return <p />;
}

// f26: a named closure handed to `setTimeout` directly: the dialect's
// fresh-stack edge, a violation.
export function NamedToScheduler() {
  const f = () => {
    onCleanup(() => {});
  };
  setTimeout(f, 0);
  return <p />;
}

// e15: a wrapper that calls its callback in its body and hands it to a
// scheduler: the census's possible-unowned edge, an obligation (ADR 0457).
export function WrapperSyncAndScheduler() {
  function both(cb: () => void) {
    cb();
    setTimeout(cb, 0);
  }
  both(() => {
    onCleanup(() => {});
  });
  return <p />;
}

// e5: a reactive write in the handed closure is legal (silent): the owner
// rules do not claim it and the write rules place it by role.
export function WrapperSchedulerWrite() {
  const [w1, setW1] = createSignal(0);
  void w1;
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  later(() => {
    setW1(7);
  });
  return <p />;
}

// Legal twin: the same closure called synchronously in the body has an owner
// edge (the ADR 0474 census) and is certified owned, silent.
export function WrapperSyncInBody() {
  function now(cb: () => void) {
    cb();
  }
  now(() => {
    onCleanup(() => {});
  });
  return <p />;
}

// Legal twin: a closure handed to a wrapper that creates its own root around
// the call: the root is the region, owned however it was entered.
export function WrapperSchedulerOwnRoot() {
  function later(cb: () => void) {
    setTimeout(cb, 0);
  }
  later(() => {
    createRoot(() => {
      onCleanup(() => {});
    });
  });
  return <p />;
}
