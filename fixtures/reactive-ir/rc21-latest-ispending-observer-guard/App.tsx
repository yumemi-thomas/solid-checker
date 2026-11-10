import * as S from "solid-js";
import { createMemo, createSignal, createTrackedEffect, isPending, latest, resolve, untrack, until } from "solid-js";

const [count] = createSignal(1);

function inner() {
  return resolve(() => count());
}

// `latest(fn)` runs `fn()` under the caller's observer, so the guard throws there as at the call.
export function LatestInTrackedEffect() {
  createTrackedEffect(() => {
    latest(() => {
      void resolve(() => count());
    });
  });
  return <p>{1}</p>;
}

export function LatestUntilInTrackedEffect() {
  createTrackedEffect(() => {
    latest(() => {
      void until(() => count());
    });
  });
  return <p>{1}</p>;
}

export function LatestInMemo() {
  const m = createMemo(() => {
    latest(() => {
      void resolve(() => count());
    });
    return 1;
  });
  return <p>{m()}</p>;
}

// The same layer through a helper the callback calls, and through the namespace import.
export function LatestHelper() {
  createTrackedEffect(() => {
    latest(() => {
      void inner();
    });
  });
  return <p>{1}</p>;
}

export function LatestNamespace() {
  createTrackedEffect(() => {
    S.latest(() => {
      void resolve(() => count());
    });
  });
  return <p>{1}</p>;
}

// `isPending(fn)` catches what `fn` throws: the guard's throw is no report (rc.21, Chrome: quiet).
export function IsPendingCatches() {
  createTrackedEffect(() => {
    isPending(() => {
      void until(() => count());
    });
    isPending(() => {
      void resolve(() => count());
    });
  });
  return <p>{1}</p>;
}

// `untrack` clears the observer; a handler has none.
export function UntrackedInMemo() {
  const m = createMemo(() => {
    untrack(() => {
      void resolve(() => count());
    });
    return 1;
  });
  return <p>{m()}</p>;
}

export function InHandler() {
  return (
    <button
      onClick={() => {
        latest(() => {
          void resolve(() => count());
        });
      }}
    >
      x
    </button>
  );
}
