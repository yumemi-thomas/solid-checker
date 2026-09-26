import * as Solid from "solid-js";
import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  onCleanup,
} from "solid-js";

const [count, setCount] = createSignal(0);
const [last, setLast] = createSignal(0);

// Module scope: no run of this apply has an owner, first or later. Proven.
createRenderEffect(
  () => count(),
  () => {
    onCleanup(() => {});
  },
);

// Owned call site: the first apply runs during the call under the component's
// owner, so the cleanup registers there; only a later run, from the flush, is
// detached. Uncertifiable, not a proven violation.
export function RenderApplyCleanup() {
  createRenderEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
  return <div />;
}

export function RenderApplyEffect() {
  createRenderEffect(
    () => count(),
    () => {
      createEffect(
        () => last(),
        () => {},
      );
    },
  );
  return <div />;
}

// A root owns the first run exactly as a component does, and a later run is
// detached. Silent today, not uncertifiable: the owner pass treats every call
// lexically inside a `createRoot` callback as root-owned, nested callbacks
// included. That approximation predates this fixture (see README.md).
createRoot(() => {
  createRenderEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
});

// The namespace spelling resolves to the same primitive.
export function NamespaceRenderApplyCleanup() {
  Solid.createRenderEffect(
    () => count(),
    () => {
      Solid.onCleanup(() => {});
    },
  );
  return <div />;
}

// Control: `createEffect` queues its apply, so every run is detached. Proven.
export function EffectApplyCleanup() {
  createEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
  return <div />;
}

// Control: the compute is owned by the render effect itself. Clean.
export function RenderComputeCleanup() {
  createRenderEffect(
    () => {
      onCleanup(() => {});
      return count();
    },
    () => {},
  );
  return <div />;
}

// Writes. The first apply answers to the component's owner and throws in dev
// when it runs during the call, which needs the compute to settle
// synchronously; the checker cannot prove that, so it reports nothing here
// rather than a violation or a certified-legal write.
export function RenderApplyWrite() {
  createRenderEffect(
    () => count(),
    (value) => {
      setLast(value);
    },
  );
  return <div>{last()}</div>;
}

// Control: a `createEffect` apply write is legal on every run.
export function EffectApplyWrite() {
  createEffect(
    () => count(),
    (value) => {
      setLast(value);
    },
  );
  return <div>{last()}</div>;
}

// Control: created in an event handler, where no owner is live, every run of
// the apply may write.
export function HandlerRenderApplyWrite() {
  return (
    <button
      onClick={() =>
        createRenderEffect(
          () => count(),
          (value) => {
            setLast(value);
          },
        )
      }
    />
  );
}

// Control: a write in the component body itself is the ordinary violation.
export function BodyWrite() {
  setCount(1);
  return <div />;
}

// Reads. The first apply runs inside the caller's synchronous extent, with the
// caller's listener current, so a helper whose only read sits there reads
// `shared` for whoever calls it: a memo calling it subscribes. The strict-read
// window the runtime opens for every apply run is still reported at the read.
const [shared] = createSignal(0);
function logShared() {
  createRenderEffect(
    () => 1,
    () => {
      console.log(shared());
    },
  );
}

// The untracked caller: reported through the helper, as for any read the
// helper performs during its call.
export function BodyReader() {
  logShared();
  return <div />;
}

// The tracked caller: the read subscribes the memo, so nothing is reported
// through the helper here.
export function MemoReader() {
  const value = createMemo(() => {
    logShared();
    return 1;
  });
  return <div>{value()}</div>;
}

// Control: a `createEffect` apply runs from the flush, never in its caller's
// extent, so the helper reads nothing for its caller.
function logSharedLater() {
  createEffect(
    () => 1,
    () => {
      console.log(shared());
    },
  );
}
export function BodyLaterReader() {
  logSharedLater();
  return <div />;
}
