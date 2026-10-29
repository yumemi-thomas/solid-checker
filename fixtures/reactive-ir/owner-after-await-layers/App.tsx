import { createEffect, getOwner, onCleanup, onSettled } from "solid-js";

// ADR 0493: an operation in function layers that run in place after an await
// runs on the await's continuation, where no owner is current.

function run(cb: () => void) {
  cb();
}

function later(cb: () => void) {
  setTimeout(cb, 0);
}

declare const flag: boolean;

// Violation: an effect in an IIFE after the await.
export function IifeAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    (() => {
      createEffect(() => 1, () => {});
    })();
  };
  void load();
  return <div />;
}

// Violation: a cleanup in a helper's callback after the await.
export function HelperAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    run(() => {
      onCleanup(() => {});
    });
  };
  void load();
  return <div />;
}

// Violation: an effect in a closure called once after the await.
export function ClosureAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    const start = () => {
      createEffect(() => 1, () => {});
    };
    start();
  };
  void load();
  return <div />;
}

// Negative: the IIFE runs before the await, under the component's owner.
export function IifeBeforeAwait() {
  const load = async () => {
    (() => {
      createEffect(() => 1, () => {});
    })();
    await Promise.resolve();
  };
  void load();
  return <div />;
}

// Uncertifiable (unchanged): the await is on one path only.
export function ConditionalAwait() {
  const load = async () => {
    if (flag) await Promise.resolve();
    (() => {
      createEffect(() => 1, () => {});
    })();
  };
  void load();
  return <div />;
}

// Known miss, outside this claim: the helper defers its callback to a timer
// through a parameter, which the climb does not cross and the owner graph does
// not place.
export function DeferredAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    later(() => {
      createEffect(() => 1, () => {});
    });
  };
  void load();
  return <div />;
}

// Not a violation (ADR 0493 review): a getOwner() guard around the layer
// skips it with no owner after the await.
export function GuardedAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    if (getOwner()) {
      run(() => {
        createEffect(() => 1, () => {});
      });
    }
  };
  void load();
  return <div />;
}

// Not a violation: a library callback may run zero times.
export function ForEachAfterAwait(props: { items: number[] }) {
  const load = async () => {
    await Promise.resolve();
    props.items.forEach(() => {
      createEffect(() => 1, () => {});
    });
  };
  void load();
  return <div />;
}

// Violation (ADR 0493 with ADR 0488): an onSettled whose callback returns a
// cleanup, in an IIFE after the await, has no owner to register it on.
export function SettledCleanupAfterAwait() {
  const load = async () => {
    await Promise.resolve();
    (() => {
      onSettled(() => () => {});
    })();
  };
  void load();
  return <div />;
}
