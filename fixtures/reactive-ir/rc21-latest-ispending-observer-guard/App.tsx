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

// ADR 0959: `isPending(async () => …)` runs the callback's code before its first await in place, but the guard's throw there is the
// async function's rejection, which `isPending` does not catch, and whether it throws at all is not modelled: an uncertifiable row
// (`async-slot-unmodelled`), never the silence of `IsPendingCatches`. Chrome (rc.21 dev) reports both codes at these sites.
export function IsPendingAsyncResolve() {
  createTrackedEffect(() => {
    isPending(async () => {
      void resolve(() => count());
    });
  });
  return <p>{1}</p>;
}

export function IsPendingAsyncUntil() {
  createTrackedEffect(() => {
    isPending(async () => {
      void until(() => count());
    });
  });
  return <p>{1}</p>;
}

export function IsPendingAsyncMemo() {
  const m = createMemo(() => {
    S.isPending(async () => {
      void resolve(() => count());
    });
    return 1;
  });
  return <p>{m()}</p>;
}

// After an await the code runs on a later task, outside any observer, but the coarse rule does not read the await's position (a rare
// shape, ADR 0959): the same uncertifiable rows, a price of the rule and no claim about the code.
export function IsPendingAsyncAfterAwait() {
  createTrackedEffect(() => {
    isPending(async () => {
      await Promise.resolve();
      void resolve(() => count());
    });
  });
  return <p>{1}</p>;
}
