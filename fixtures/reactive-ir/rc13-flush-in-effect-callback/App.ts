// SC2008 flush-in-effect-callback. A user effect's apply callback is queued
// and run by `runEffect` inside the flush that drains effects, between
// `setEffectCallback(true)` and `setEffectCallback(false)` (rc.13
// `dist/dev.js:1876-1906`). `flush`'s running-flush branch then tests that
// marker, reports `FLUSH_IN_EFFECT_CALLBACK` (a warning) and returns without
// draining (`dist/dev-shared.js:2680-2698`); the production build returns at
// the same branch silently. Positive: the call runs on the apply's own stack.
// Negative: anything else, including the compute function (no marker), calls
// deferred to a timer or microtask, and code no layer proves runs there.
import {
  createEffect,
  createRenderEffect,
  createRoot,
  createSignal,
  flush,
  flush as drain,
  untrack,
} from "solid-js";
import * as Solid from "solid-js";
import { flush as signalsFlush } from "@solidjs/signals";
import { commitFromOtherFile } from "./commit";

// The cases sit in a root so each effect has an owner.
createRoot(() => {
  const [value, setValue] = createSignal(1);

  // Positive: directly in the apply callback.
  createEffect(value, () => {
    flush();
  });

  // Positive: the callback form is refused the same way: the inner `flush()`
  // that `flush(fn)` ends with runs while the effect marker is up.
  createEffect(value, () => {
    flush(() => setValue(2));
  });

  // Positive: after writes, with the apply's own parameters.
  createEffect(
    () => value(),
    (next, previous) => {
      setValue(next + (previous ?? 0));
      flush();
    },
    { name: "write-then-flush" },
  );

  // Positive: an alias and a namespace member are the same export.
  createEffect(value, () => {
    drain();
  });
  createEffect(value, () => {
    Solid.flush();
  });

  // Negative: `flush` imported from `@solidjs/signals` itself resolves to a
  // declaration this proof cannot tie to the instance `createEffect` runs on
  // (a second installed copy has its own queue): not claimed.
  createEffect(value, () => {
    signalsFlush();
  });

  // Positive: the claim is that the nested flush drains nothing (the queue is
  // already running). A synchronous render effect inside the apply lowers the
  // dev warning's marker before the flush, so the dev warning may be absent;
  // the no-op is not.
  createEffect(value, () => {
    createRenderEffect(
      () => 1,
      () => {},
    );
    flush();
  });
  function mountRenderEffect(): void {
    createRenderEffect(
      () => 2,
      () => {},
    );
  }
  createEffect(value, () => {
    mountRenderEffect();
    flush();
  });
  createEffect(value, () => {
    flush(() => {
      createRenderEffect(
        () => 3,
        () => {},
      );
    });
  });

  // Positive: a conditional call is claimed on the engine's feasible-path
  // premise: nothing here decides the condition.
  createEffect(value, (next) => {
    if (next > 100) flush();
  });

  // Positive: a function declaration the apply calls.
  function commitDeclared(): void {
    flush();
  }
  createEffect(value, () => {
    commitDeclared();
  });

  // Positive: a chain of const-bound arrows, each called from the previous.
  const innerArrow = (): void => {
    flush();
  };
  const outerArrow = (): void => innerArrow();
  createEffect(value, () => {
    outerArrow();
  });

  // Positive: a helper in another file.
  createEffect(value, () => {
    commitFromOtherFile();
  });

  // Positive: an immediately invoked function runs where it is written.
  createEffect(value, () => {
    (() => {
      flush();
    })();
  });

  // Positive: `untrack` runs its callback during the call.
  createEffect(value, () => {
    untrack(() => flush());
  });

  // Positive: `createRoot` runs its init during the call.
  createEffect(value, () => {
    createRoot(() => {
      flush();
    });
  });

  // Positive: an array method the audited rows model as invoking its callback
  // before it returns.
  createEffect(value, () => {
    [1, 2].forEach(() => {
      flush();
    });
  });

  // Positive: an async helper runs on the caller's stack up to its first await.
  async function loadThenSettle(): Promise<void> {
    flush();
    await Promise.resolve();
  }
  createEffect(value, () => {
    void loadThenSettle();
  });

  // Positive: a helper also called from a handler is still reached from the
  // effect by this call.
  function sharedHelper(): void {
    flush();
  }
  function onClickShared(): void {
    sharedHelper();
  }
  void onClickShared;
  createEffect(value, () => {
    sharedHelper();
  });

  // Negative: the compute function runs before the marker is raised, and a
  // nested flush there is a silent no-op, not FLUSH_IN_EFFECT_CALLBACK.
  createEffect(
    () => {
      flush();
      return value();
    },
    () => {},
  );

  // Negative: an event handler is not inside the flush that runs effects.
  function onClick(): void {
    flush();
  }
  void onClick;

  // Negative: a timer or microtask created in the apply runs after the effect
  // phase has ended, when `flush()` drains normally.
  createEffect(value, () => {
    setTimeout(() => flush());
    queueMicrotask(() => flush());
    Promise.resolve().then(() => flush());
  });

  // Negative: a user function named `flush` is not Solid's.
  createEffect(value, () => {
    const flush = (): void => {};
    flush();
  });
  function localFlush(flush: () => void): void {
    createEffect(value, () => {
      flush();
    });
  }
  void localFlush;

  // Negative: a member that is not the export.
  const scheduler = { flush(): void {} };
  createEffect(value, () => {
    scheduler.flush();
  });

  // Negative: `createRenderEffect`'s first run is synchronous at creation, with
  // no flush in progress, so a flush there still drains; later runs report, and
  // which one is running is not proven here.
  createRenderEffect(value, () => {
    flush();
  });

  // Negative: a helper no call from an effect reaches.
  function onlyFromHandler(): void {
    flush();
  }
  function onKey(): void {
    onlyFromHandler();
  }
  void onKey;

  // Negative: a helper whose only call from the effect is a deferred one.
  function deferredHelper(): void {
    flush();
  }
  createEffect(value, () => {
    setTimeout(deferredHelper);
    setTimeout(() => deferredHelper());
  });

  // Negative: a generator helper runs none of its body when called.
  function* generatedFlush(): Generator<void> {
    flush();
    yield;
  }
  createEffect(value, () => {
    generatedFlush();
  });

  // Negative: past its first await an async helper runs from a microtask.
  async function settleThenFlush(): Promise<void> {
    await Promise.resolve();
    flush();
  }
  createEffect(value, () => {
    void settleThenFlush();
  });

  // Negative: a `let` helper may have been replaced before the call.
  let replaceable = (): void => {
    flush();
  };
  replaceable = (): void => {};
  createEffect(value, () => {
    replaceable();
  });

  // Negative: a member call whose receiver is a factory's result. The function
  // it names is the factory's own, but nothing proves the call runs it: the
  // returned object may be replaced or extended (ADR 0279).
  function makeApi() {
    function commitMember(): void {
      flush();
    }
    return { commitMember };
  }
  const api = makeApi();
  createEffect(value, () => {
    api.commitMember();
  });

  // Negative: code after an unconditional return or throw never runs.
  createEffect(value, () => {
    return;
    flush();
  });
  createEffect(value, () => {
    throw new Error("boom");
    flush();
  });

  // Negative: a callback of an array literal written empty is never invoked.
  createEffect(value, () => {
    ([] as number[]).forEach(() => flush());
    [].filter(() => {
      flush();
      return true;
    });
  });

  // Negative: a compute that always throws leaves the apply unrun.
  createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      flush();
    },
  );

  // Negative: abrupt completion before the call. Each leaves no normal path to
  // the `flush()`: a `try` whose blocks only return, an infinite loop, a
  // `switch` whose every arm returns, a call that never returns (a `never`
  // signature, a local `never` arrow), and an inline callback that only throws
  // over a literal array with a first element.
  function fail(): never {
    throw new Error("always");
  }
  createEffect(value, () => {
    try {
      return;
    } finally {
      void 0;
    }
    flush();
  });
  createEffect(value, () => {
    for (;;) {}
    flush();
  });
  createEffect(value, () => {
    switch (1) {
      default:
        return;
    }
    flush();
  });
  createEffect(value, () => {
    const never = (): never => {
      throw new Error();
    };
    never();
    flush();
  });
  createEffect(value, () => {
    fail();
    flush();
  });
  createEffect(value, () => {
    [1].forEach(() => {
      throw new Error();
    });
    flush();
  });

  // Negative: a compute that never returns leaves the apply unrun: a call of a
  // `never` function, a const arrow, a declaration, and a helper it returns.
  const throwingCompute = (): number => {
    throw new Error("always");
  };
  function throwingDeclaration(): number {
    throw new Error("always");
  }
  createEffect(
    () => fail(),
    () => {
      flush();
    },
  );
  createEffect(throwingCompute, () => {
    flush();
  });
  createEffect(throwingDeclaration, () => {
    flush();
  });
  createEffect(
    () => {
      value();
      return throwingDeclaration();
    },
    () => {
      flush();
    },
  );

  // Positive: a compute that can return, written as a named function or a
  // member, is analyzed or assumed to; a throw guarded by a condition still
  // returns on the other path.
  function guardedCompute(): number {
    if (value() < 0) throw new Error("negative");
    return value();
  }
  createEffect(guardedCompute, () => {
    flush();
  });

  // Negative: a cleanup the apply returns runs at the next run (inside the
  // marker) and at disposal (outside it): no single answer, so none is claimed.
  createEffect(value, () => () => {
    flush();
  });

  // Negative: a closure that is stored, not called, in the apply.
  const stored: Array<() => void> = [];
  createEffect(value, () => {
    stored.push(() => flush());
  });

  // Negative: an apply named rather than written is not placed here.
  function namedApply(): void {
    flush();
  }
  createEffect(value, namedApply);

  // Negative: the bundle form's `effect` is the same phase, but its position is
  // not one this proof places.
  createEffect(value, {
    effect: () => {
      flush();
    },
    error: () => {},
  });

  // Negative: a deferred first run of a constant compute never applies.
  createEffect(
    () => 1,
    () => {
      flush();
    },
    { defer: true },
  );

});

// Negative: module scope runs outside every effect.
flush();
