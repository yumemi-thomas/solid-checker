import { createEffect, createSignal, getOwner, onCleanup } from "solid-js";

const [count] = createSignal(0);

// Guarded: the operation runs only once the same invocation has seen an owner,
// even when it sits in an immediately invoked literal (ADR 0347 amendment). Each helper is
// called at module scope, the context a detached call sees.
function guardedIife() {
  if (getOwner()) {
    (() => {
      createEffect(count, () => {});
    })();
  }
}
guardedIife();

function guardedAnd() {
  getOwner() &&
    (() => {
      createEffect(count, () => {});
    })();
}
guardedAnd();

function guardedTernary() {
  return getOwner()
    ? (() => {
        onCleanup(() => {});
      })()
    : null;
}
guardedTernary();

function guardedNested() {
  if (getOwner()) {
    (() => {
      (() => {
        createEffect(count, () => {});
      })();
    })();
  }
}
guardedNested();

// Controls. Each is reported, because the owner is not current where it runs.

// No guard at all.
function unguardedIife() {
  (() => {
    createEffect(count, () => {});
  })();
}
unguardedIife();

// The literal sits in the branch the guard does not take.
function elseBranch() {
  if (getOwner()) {
    // guarded branch: nothing here
  } else {
    (() => {
      createEffect(count, () => {});
    })();
  }
}
elseBranch();

// `getOwner` is not Solid's here, so the test proves nothing.
function getOwnerLocal() {
  return true;
}
function shadowedGuard() {
  if (getOwnerLocal()) {
    (() => {
      createEffect(count, () => {});
    })();
  }
}
shadowedGuard();

// An owner seen before an `await` is not current after it.
async function awaitedGuard() {
  if (getOwner()) {
    await Promise.resolve();
    createEffect(count, () => {});
  }
}

// An async literal's code after its first `await` is a continuation.
function guardedAsyncIife() {
  if (getOwner()) {
    (async () => {
      await Promise.resolve();
      createEffect(count, () => {});
    })();
  }
}

// Rendered by a component, the owner is current when the guards run. The code
// after the first `await` in each of these is a continuation with no owner.
export function Host() {
  awaitedGuard();
  guardedAsyncIife();
  return null;
}

// A component's own body is owned: the same guarded literal stays silent.
export function Comp() {
  if (getOwner()) {
    (() => {
      createEffect(count, () => {});
    })();
  }
  return null;
}
