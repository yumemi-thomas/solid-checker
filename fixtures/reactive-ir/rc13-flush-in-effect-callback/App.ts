// SC2008 flush-in-effect-callback. A user effect's apply callback is queued
// and run by `runEffect` inside the flush that drains effects, between
// `setEffectCallback(true)` and `setEffectCallback(false)` (rc.13
// `dist/dev.js:1876-1906`). `flush`'s running-flush branch then tests that
// marker, reports `FLUSH_IN_EFFECT_CALLBACK` (a warning) and returns without
// draining (`dist/dev-shared.js:2680-2698`); the production build returns at
// the same branch silently. Positive: the call runs on the apply's own stack.
// The claim is universal: whenever the call runs it runs on an apply's stack,
// so a call in dead code is reported too. Negative: any call that may run
// elsewhere, including the compute function (no marker), calls deferred to a
// timer or microtask, and helpers with a caller no layer proves.
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

// ADR 0459: a `const` destructuring of the namespace binds the export itself;
// a `let` one may be reassigned, so it binds nothing this proof names.
const { flush: destructuredFlush } = Solid;
let { flush: reassignableFlush } = Solid;

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
  createEffect(value, () => {
    destructuredFlush();
  });

  // Negative: the `let` slot names nothing.
  createEffect(value, () => {
    reassignableFlush();
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

  // Positive: a conditional call, whatever the condition.
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

  // Negative: a helper in another file is exported, so code this build cannot
  // see may call it from outside any effect.
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
  const items: number[] = [1, 2];
  createEffect(value, () => {
    items.forEach(() => {
      flush();
    });
  });
  createEffect(value, () => {
    items.reduce((accumulator) => {
      flush();
      return accumulator;
    }, 0);
  });

  // Positive: an async helper runs on the caller's stack up to its first await.
  async function loadThenSettle(): Promise<void> {
    flush();
    await Promise.resolve();
  }
  createEffect(value, () => {
    void loadThenSettle();
  });

  // Negative: a helper also called from a handler may flush outside the
  // effect, so one call from an effect proves nothing.
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

  // Negative: a helper no call from an effect reaches (its only caller is a
  // handler).
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

  // Negative: a helper an effect calls and an alias also reaches.
  function aliasedHelper(): void {
    flush();
  }
  const alias = aliasedHelper;
  void alias;
  createEffect(value, () => {
    aliasedHelper();
  });

  // Negative: a helper called from the apply and from the module body.
  function moduleAndEffect(): void {
    flush();
  }
  createEffect(value, () => {
    moduleAndEffect();
  });
  moduleAndEffect();

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

  // Positive: dead code is reported, because the call is pointless wherever it
  // runs: after a return, under a constant condition, in an apply whose compute
  // never returns, and in an effect that is never registered.
  createEffect(value, () => {
    return;
    flush();
  });
  createEffect(value, () => {
    if (false) flush();
  });
  createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      flush();
    },
  );
  function neverCalled() {
    createEffect(value, () => {
      flush();
    });
  }
  void neverCalled;

  // Negative: a deferred first run of a constant compute never applies, and the
  // engine reports nothing inside an apply it proves absent (ADR 0302).
  createEffect(
    () => 1,
    () => {
      flush();
    },
    { defer: true },
  );

  // Negative: past its first await the apply itself runs from a microtask.
  createEffect(value, async () => {
    await Promise.resolve();
    flush();
  });
});

// Negative: module scope runs outside every effect.
flush();
