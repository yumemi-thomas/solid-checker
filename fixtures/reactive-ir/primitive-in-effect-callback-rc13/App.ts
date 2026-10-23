// The rc.13 twin of the rc.17 fixture: the same source, `createDeferred` aside
// (rc.13 does not export it). SC4005 on the rc.13 triple. A user effect's
// apply runs from the flush, not under any owner, and rc.17's dev build
// refuses to create a root, memo or effect there: `createOwner` and
// `setupComputedNode` both call `assertNotInEffectCallback()`
// (`@solidjs/signals` `dist/dev-shared.js:1496`, `:4927`), which throws
// PRIMITIVE_IN_EFFECT_CALLBACK while `inEffectCallback` is up and the context
// is null (`:3947-3957`). The production build creates the computation and
// nothing owns it. rc.13 has no such assertion (0 hits in its bytes), so every
// call below is quiet there; the rc.17 fixture has the same source with the
// positives marked.
import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  createTrackedEffect,
  getOwner,
  onCleanup,
  runWithOwner,
  untrack,
} from "solid-js";
import * as Solid from "solid-js";
import { memoFromOtherFile } from "./memo";

// A non-exported helper that only applies call: the call runs on the stack of
// each of them, so it is as unowned as a call written in the apply.
function rootHelper(): void {
  createRoot(() => {});
}
const memoHelper = () => createMemo(() => 1);
const chainedHelper = () => rootHelper();
async function memoBeforeFirstAwait(): Promise<void> {
  createMemo(() => 1);
  await Promise.resolve();
}

// A helper that something outside the effect phase also calls, or that
// escapes, may run with an owner: not claimed.
function sharedHelper(): void {
  createMemo(() => 1);
}
function aliasedHelper(): void {
  createMemo(() => 1);
}
const alias = aliasedHelper;
function handlerHelper(): void {
  createRoot(() => {});
}
async function memoAfterAwait(): Promise<void> {
  await Promise.resolve();
  createMemo(() => 1);
}
function* generatorHelper(): Generator<void> {
  createMemo(() => 1);
  yield;
}
let reassignedHelper = (): void => {
  createMemo(() => 1);
};
reassignedHelper = () => {};

// The module body is not an apply: a helper called from here has a non-effect
// caller.
sharedHelper();
export function onClick(): void {
  handlerHelper();
}

createRoot(() => {
  const [value] = createSignal(1);
  const owner = getOwner();

  // Positive: a root directly in the apply.
  createEffect(value, () => {
    createRoot(() => {});
  });

  // Positive: a memo directly in the apply.
  createEffect(value, () => {
    createMemo(() => value() + 1);
  });

  // Positive: through a project helper only applies call.
  createEffect(value, () => {
    rootHelper();
  });
  createEffect(value, () => {
    memoHelper();
  });
  createEffect(value, () => {
    chainedHelper();
  });

  // Positive: an async helper runs on the apply's stack up to its first await.
  createEffect(value, () => {
    void memoBeforeFirstAwait();
  });

  // Positive: a namespace member is the same export.
  createEffect(value, () => {
    Solid.createMemo(() => value() + 1);
  });

  // Positive: `untrack` and an immediately invoked function keep the owner
  // none.
  createEffect(value, () => {
    untrack(() => createMemo(() => value() + 1));
  });
  createEffect(value, () => {
    (() => {
      createRoot(() => {});
    })();
  });

  // Positive: the apply's own parameters, a memo inside a branch (the call is
  // universal: it is unowned wherever it runs).
  createEffect(
    () => value(),
    (next, previous) => {
      if (next > (previous ?? 0)) {
        createMemo(() => next);
      }
    },
    { name: "branch" },
  );

  // Positive: only the outer root. The inner memo is in a root callback the
  // outer call never reaches, so it is neither claimed nor owned by anything
  // this rule can see.
  createEffect(value, () => {
    createRoot(() => {
      createMemo(() => 1);
    });
  });

  // Positive: only the outer memo is created without an owner; the inner root
  // runs in the memo's own computation.
  createEffect(value, () => {
    createMemo(() => createRoot(() => {}));
  });

  // Negative: a signal is inert there and the runtime lets it be created.
  createEffect(value, () => {
    createSignal(1);
  });

  // Negative: `onCleanup` is SC4001's claim (NO_OWNER_CLEANUP), and the
  // runtime does not throw.
  createEffect(value, () => {
    onCleanup(() => {});
  });

  // Negative: parented explicitly by an owner captured outside the effect.
  createEffect(value, () => {
    runWithOwner(owner, () => createMemo(() => value() + 1));
  });
  createEffect(value, () => {
    runWithOwner(owner, () => {
      createRoot(() => {});
    });
  });

  // Negative: the effect constructors reach the same assertion but are
  // SC4001's (`NO_OWNER_EFFECT`); no second finding.
  createEffect(value, () => {
    createEffect(value, () => {});
  });
  createEffect(value, () => {
    createRenderEffect(value, () => {});
  });
  createEffect(value, () => {
    createTrackedEffect(() => {});
  });

  // Negative: after the first await the stack is the continuation's, where
  // the marker is down.
  createEffect(value, () => {
    void memoAfterAwait();
  });

  // Negative: a callback the apply only hands to a timer or microtask.
  createEffect(value, () => {
    setTimeout(() => createMemo(() => 1), 0);
    queueMicrotask(() => {
      createRoot(() => {});
    });
  });
  createEffect(value, () => {
    const later = () => createMemo(() => 1);
    void later;
  });

  // Negative: a createRenderEffect apply's first run is synchronous at
  // creation, so the rule cannot tell the first run from a later one.
  createRenderEffect(value, () => {
    createMemo(() => value() + 1);
    createRoot(() => {});
  });

  // Negative: the compute function runs inside the effect's own computation,
  // an owner.
  createEffect(
    () => {
      createMemo(() => 1);
      return value();
    },
    () => {},
  );

  // Negative: helpers with a caller outside the effect phase, an alias, a
  // generator, a reassignable binding, an exported helper, or a member.
  createEffect(value, () => {
    sharedHelper();
    alias();
    handlerHelper();
    void generatorHelper();
    reassignedHelper();
    memoFromOtherFile(value);
  });

  // Negative: an apply passed by name is not placed.
  const namedApply = () => {
    createMemo(() => 1);
  };
  createEffect(value, namedApply);

  // Negative: a shadowing local with the primitive's name is no primitive.
  createEffect(value, () => {
    const createMemo = (fn: () => number): number => fn();
    createMemo(() => 1);
  });

  // Negative: a member of an object that merely holds a function.
  const holder = { make: () => createMemo(() => 1) };
  createEffect(value, () => {
    holder.make();
  });
});
