import {
  action, createEffect, createMemo, createRoot, createSignal, createTrackedEffect, flush, getOwner, isPending, latest,
  onCleanup, onSettled, resolve, runWithOwner, untrack,
} from "solid-js";
import * as S from "solid-js";

// Positives. The callback of a primitive the dialect says runs in place (`untrack`, `latest`, `isPending`, `flush(fn)`,
// `runWithOwner(owner, fn)`) runs on the stack of its call, under the owner the call has: the leaf owner's.
export function UntrackCleanup() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    n();
    untrack(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

export function LatestCreation() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    n();
    latest(() => {
      createMemo(() => n());
    });
  });
  return <div />;
}

// `isPending` catches what its callback throws, but the leaf owner's diagnostic is emitted before the throw.
export function IsPendingCleanupInSettled() {
  onSettled(() => {
    isPending(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

// `getOwner()` in the leaf's own function is the owner the `runWithOwner` call runs under: the leaf's.
export function OwnerCapturedInTheLeaf() {
  createTrackedEffect(() => {
    const owner = getOwner();
    runWithOwner(owner, () => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

// `flush(fn)` runs `fn` first and drains after: the cleanup is refused, and so is the flush.
export function FlushCallback() {
  createTrackedEffect(() => {
    flush(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

// Layers between: an untrack in an untrack, an IIFE in an untrack.
export function NestedLayers() {
  createTrackedEffect(() => {
    untrack(() => {
      untrack(() => {
        onCleanup(() => {});
      });
      (() => {
        onCleanup(() => {});
      })();
    });
  });
  return <div />;
}

// A helper called in the callback performs it in its synchronous extent: reported at the helper call.
function registersCleanup() {
  onCleanup(() => {});
}

export function HelperInUntrack() {
  createTrackedEffect(() => {
    untrack(() => registersCleanup());
  });
  return <div />;
}

// A callback named by value is a function called there.
function namedCallback() {
  onCleanup(() => {});
}

export function NamedCallback() {
  createTrackedEffect(() => {
    untrack(namedCallback);
  });
  return <div />;
}

// The same inside a helper: reported at the call of the helper from the leaf.
function untracksACleanup() {
  untrack(() => {
    onCleanup(() => {});
  });
}

export function SlotInsideAHelper() {
  createTrackedEffect(() => {
    untracksACleanup();
  });
  return <div />;
}

// `untrack` clears the observer, so `resolve` meets no observer guard and creates a root under the leaf owner.
export function UntrackResolve() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    untrack(() => {
      void resolve(() => n());
    });
  });
  return <div />;
}

export function NamespaceImport() {
  S.createTrackedEffect(() => {
    S.untrack(() => {
      S.onCleanup(() => {});
    });
  });
  return <div />;
}

// JSX in the callback creates under the leaf owner as well.
export function JsxInUntrack() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    untrack(() => {
      void (<span>{n()}</span>);
    });
  });
  return <div />;
}

// Uncertifiable. A local alias of the primitive is no proven identity (ADR 0711); a callback the walk cannot read
// may do anything; an owner that is a `let` the code reassigns is not followed.
const alias = untrack;

export function AliasedPrimitive() {
  createTrackedEffect(() => {
    alias(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

declare const unreadable: () => void;

export function UnreadableCallback() {
  createTrackedEffect(() => {
    untrack(unreadable);
  });
  return <div />;
}

export function OwnerThatMayBeAnything() {
  createTrackedEffect(() => {
    let owner = getOwner();
    owner = null;
    runWithOwner(owner, () => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

// Negatives. No leaf owner is current in `runWithOwner(null, …)`; an owner captured in the component body is the
// component's; a callback handed to a timer or a listener runs later; `isPending` catches `resolve`'s and `flush`'s
// plain throws, which no diagnostic precedes; `latest` of an accessor runs no code of the program's.
export function DetachedOwner() {
  createTrackedEffect(() => {
    runWithOwner(null, () => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

export function ComponentOwner() {
  const owner = getOwner();
  createTrackedEffect(() => {
    runWithOwner(owner, () => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

export function DeferredCallbacks() {
  createTrackedEffect(() => {
    setTimeout(() => {
      untrack(() => {
        createRoot(() => {});
      });
    });
  });
  return <div />;
}

export function CaughtThrows() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    isPending(() => {
      void resolve(() => n());
    });
    isPending(() => {
      flush();
    });
  });
  return <div />;
}

export function AnAccessorHandedToLatest() {
  const [n] = createSignal(1);
  createTrackedEffect(() => {
    latest(n);
  });
  return <div />;
}

// `createRoot` under a leaf owner is refused at the call, and its callback never runs: one finding, at the call.
export function RootInUntrack() {
  createTrackedEffect(() => {
    untrack(() => {
      createRoot(() => {
        onCleanup(() => {});
      });
    });
  });
  return <div />;
}
