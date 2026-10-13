import * as Solid from "solid-js";
import {
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  createStore,
  flush,
  onSettled,
  refresh,
  untrack,
} from "solid-js";

const [count, setCount] = createSignal(0);
const [state, setState] = createStore({ value: 0 });
const [internal, setInternal] = createSignal(0, { ownedWrite: true });

// Directly in a module-level root body: the root is the ambient owner.
createRoot(() => {
  const doubled = createMemo(() => count() * 2);
  // SC2001 on every release: `setSignal` never exempted roots.
  setCount(1);
  // SC2001 only where the store setter's guard rejects roots (rc.9).
  setState((draft) => {
    draft.value = 1;
  });
  // SC2001 on every release: `refresh`'s guard has no root exemption.
  refresh(doubled);
  // None: the source opted out with `ownedWrite`.
  setInternal(1);
});

// The dispose-taking form and the namespace spelling are the same root.
Solid.createRoot((dispose) => {
  setCount(2);
  setState((draft) => {
    draft.value = 2;
  });
  return dispose;
});

// `untrack` clears tracking and keeps the root: the same answer as the body.
createRoot(() => {
  untrack(() => setCount(3));
  untrack(() =>
    setState((draft) => {
      draft.value = 3;
    }),
  );
});

// A helper only a root body calls runs under that root.
function writeBoth() {
  setCount(4);
  setState((draft) => {
    draft.value = 4;
  });
}
createRoot(() => writeBoth());

// A root created where writes are otherwise legal (an effect apply, which runs
// with no owner) or illegal (a memo compute): either way the write answers to
// the new root.
createRoot(() => {
  createEffect(
    () => count(),
    () => {
      createRoot(() => {
        setCount(5);
        setState((draft) => {
          draft.value = 5;
        });
      });
    },
  );
  createMemo(() => {
    createRoot(() => {
      setCount(6);
      setState((draft) => {
        draft.value = 6;
      });
    });
    return count();
  });
});

// Callbacks nested in a root body run under their own owner, or none, so none
// of these is a root-body write.
createRoot(() => {
  // An effect apply runs from the flush, with no owner: legal.
  createEffect(
    () => count(),
    (value) => {
      setCount(value + 1);
      setState((draft) => {
        draft.value = value;
      });
    },
  );
  // onSettled under a root is a leaf: legal.
  onSettled(() => {
    setCount(7);
    setState((draft) => {
      draft.value = 7;
    });
  });
  // An event listener runs from the event loop: legal.
  document.addEventListener("click", () => {
    setCount(8);
    setState((draft) => {
      draft.value = 8;
    });
  });
  // A memo compute in the root is the memo's scope, not the root's: SC2001
  // for both setters on every release, the arm that predates this one.
  createMemo(() => {
    setCount(9);
    setState((draft) => {
      draft.value = 9;
    });
    return count();
  });
});

// `flush(fn)` runs `fn` inline under the root too, keeping its owner as
// `untrack` does, and the runtime throws there (probed): SC2001 on every
// release.
createRoot(() => {
  flush(() => setCount(10));
});

// Module scope, outside any root: legal.
setCount(11);
setState((draft) => {
  draft.value = 11;
});

export { internal, state };
