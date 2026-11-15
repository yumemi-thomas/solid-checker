import { createMemo, createSignal, createTrackedEffect, flush, isPending, latest, onSettled, resolve, until, untrack } from "solid-js";

const [count] = createSignal(1);

function inner() {
  void resolve(() => count());
}

function innerUntil() {
  void until(() => count());
}

// `isPending(fn)` catches what `fn` throws, so the observer guard's throw in a callback it runs is no report (rc.21, Chrome: quiet), whether the
// callback is written inline or named. The named forms below were reported by the leaf walk as a violation at the `isPending` call.
export function NamedConst() {
  createTrackedEffect(() => {
    const cb = () => {
      void resolve(() => count());
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}

export function NamedConstUntil() {
  createTrackedEffect(() => {
    const cb = () => {
      void until(() => count());
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}

export function NamedFunctionDeclaration() {
  createTrackedEffect(() => {
    function cb() {
      void resolve(() => count());
    }
    isPending(cb);
  });
  return <p>{1}</p>;
}

// A name another binding holds, a callback forwarded through a parameter, and a named callback that calls a helper.
export function NamedAlias() {
  createTrackedEffect(() => {
    const run = inner;
    isPending(run);
  });
  return <p>{1}</p>;
}

function takes(f: () => unknown) {
  return isPending(f);
}

export function NamedForwarded() {
  createTrackedEffect(() => {
    const cb = () => {
      void resolve(() => count());
    };
    takes(cb);
  });
  return <p>{1}</p>;
}

export function NamedCallsHelper() {
  createTrackedEffect(() => {
    const cb = () => {
      innerUntil();
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}

// A memo's compute is no leaf: the call's own answer (the slot takes the throw) was already silent.
export function NamedInMemo() {
  const m = createMemo(() => {
    const cb = () => {
      void resolve(() => count());
    };
    isPending(cb);
    return count();
  });
  return <p>{m()}</p>;
}

// `flush()` in a named callback of `isPending` is the same plain throw (ADR 0959's `nm-*`).
export function NamedFlush() {
  createTrackedEffect(() => {
    const cb = () => {
      flush();
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}

// What the slot does not catch stays reported. In `onSettled` no observer is active: `resolve` is the creation the leaf owner refuses, a diagnostic emitted
// before anything throws (Chrome: PRIMITIVE_IN_FORBIDDEN_SCOPE), so `isPending` takes nothing and the row stays.
export function NamedInSettled() {
  onSettled(() => {
    const cb = () => {
      void resolve(() => count());
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}

// `latest(fn)` and `untrack(fn)` do not catch: `latest` keeps the observer (the guard throws, SC2004), `untrack` clears it (the creation is refused).
export function NamedLatest() {
  createTrackedEffect(() => {
    const cb = () => {
      void resolve(() => count());
    };
    latest(cb);
  });
  return <p>{1}</p>;
}

export function NamedUntrack() {
  createTrackedEffect(() => {
    const cb = () => {
      void resolve(() => count());
    };
    untrack(cb);
  });
  return <p>{1}</p>;
}

// An async callback's throw is the function's rejection, which `isPending` does not catch: an obligation, as the literal `isPending(async () => …)` is (ADR 0959).
export function NamedAsync() {
  createTrackedEffect(() => {
    const cb = async () => {
      void resolve(() => count());
    };
    isPending(cb);
  });
  return <p>{1}</p>;
}
