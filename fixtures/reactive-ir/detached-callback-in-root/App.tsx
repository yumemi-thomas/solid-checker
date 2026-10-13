import * as Solid from "solid-js";
import {
  createEffect,
  createReaction,
  createRoot,
  createSignal,
  onCleanup,
  onSettled,
  runWithOwner,
} from "solid-js";

const [count] = createSignal(0);

// Each callback below is written inside a `createRoot` callback but runs with
// no owner on every run, so the root's owner never answers for it: the
// operation is judged on the owner graph, where it is proven unowned.

// A `createEffect` apply runs from the flush.
createRoot(() => {
  createEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
});

// The namespace spelling resolves to the same primitives.
createRoot(() => {
  Solid.createEffect(
    () => count(),
    () => {
      Solid.onCleanup(() => {});
    },
  );
});

// An effect created in that apply has no owner either.
createRoot(() => {
  createEffect(
    () => count(),
    () => {
      createEffect(
        () => count(),
        () => {},
      );
    },
  );
});

// An event handler runs from the event's dispatch.
createRoot(() => (
  <button
    onClick={() => {
      onCleanup(() => {});
    }}
  />
));

// A `createReaction` invalidation runs from the flush.
createRoot(() => {
  const track = createReaction(() => {
    onCleanup(() => {});
  });
  track(() => count());
});

// `runWithOwner(null, fn)` runs `fn` with no owner, whatever the call site.
createRoot(() => {
  runWithOwner(null, () => {
    onCleanup(() => {});
  });
});

// An `onSettled` in the apply is out-of-band: its returned cleanup has no
// owner to be honored under.
createRoot(() => {
  createEffect(
    () => count(),
    () => {
      onSettled(() => () => {});
    },
  );
});

// A helper declared in the root and called from both the root body and the
// apply: the apply's call is a proven unowned invocation.
createRoot(() => {
  function register() {
    onCleanup(() => {});
  }
  register();
  createEffect(
    () => count(),
    () => {
      register();
    },
  );
});

// Controls: each stays silent.
createRoot(() => {
  // A root created in the apply owns what it contains.
  createEffect(
    () => count(),
    () => {
      createRoot(() => {
        onCleanup(() => {});
      });
    },
  );
  // A compiled JSX child in the apply is evaluated by the render effect the
  // compiler generates for it, which owns it.
  createEffect(
    () => count(),
    () => {
      void (<div>{(onCleanup(() => {}), "label")}</div>);
    },
  );
  // An effect compute is owned by the effect.
  createEffect(
    () => {
      onCleanup(() => {});
      return count();
    },
    () => {},
  );
  // An `onSettled` in the root body is owned; so is its returned cleanup.
  onSettled(() => () => {});
});

// A timer callback runs from a task queue with no owner: the reviewed
// fresh-stack schedulers give their callback an unowned owner edge (the
// `fresh-stack-scheduler-owner` fixture owns that claim).
createRoot(() => {
  setTimeout(() => {
    onCleanup(() => {});
  }, 0);
});
