import { createEffect, createMemo } from "solid-js";

// ADR 1159: an async function the apply runs in place: what it creates after an `await` every run reaches runs from a microtask, off the apply's
// synchronous run: silent (`effect-creation-after-await`). Before the `await` it is on the apply's stack: SC6002. After an `await` only one arm
// reaches, it may run on the apply's stack: no silence from the await.
export function ApplyAfterAwait() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        await Promise.resolve();
        createMemo(() => 16);
      })();
    },
  );
  return <p>after</p>;
}
export function ApplyBeforeAwait() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        createMemo(() => 17);
        await Promise.resolve();
      })();
    },
  );
  return <p>before</p>;
}
export function ApplyBranchAwait(props: { wait: boolean }) {
  createEffect(
    () => props.wait,
    (wait) => {
      void (async () => {
        if (wait) await Promise.resolve();
        createMemo(() => 18);
      })();
    },
  );
  return <p>branch</p>;
}

// A handler handed to a component, which calls it in an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK, covered by SC6002.
function Kid(props: { onClick: () => void }) {
  createEffect(
    () => 0,
    () => {
      props.onClick();
    },
  );
  return <p>kid</p>;
}
export function PropHandler() {
  return (
    <Kid
      onClick={() => {
        createMemo(() => 15);
      }}
    />
  );
}
