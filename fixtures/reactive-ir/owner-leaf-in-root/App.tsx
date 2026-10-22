import { createMemo, createRoot, createSignal, createTrackedEffect, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

export function RootNestedSettled() {
  // An onSettled inside an owned onSettled is queued out of band: its callback
  // runs with no owner, so the onCleanup there is NO_OWNER_CLEANUP (SC4001). The
  // root around the leaves does not own that callback.
  createRoot(() => {
    onSettled(() => {
      onSettled(() => {
        onCleanup(() => {});
      });
    });
  });
  return <i />;
}

export function RootTrackedSettled() {
  // The same queue under a tracked effect: its body is a leaf, so the nested
  // onSettled is out of band and its onCleanup is SC4001's claim.
  createRoot(() => {
    createTrackedEffect(() => {
      count();
      onSettled(() => {
        onCleanup(() => {});
      });
    });
  });
  return <i />;
}

export function RootSettledCleanup() {
  // The queued callback returns a cleanup with no owner: SETTLED_CLEANUP_UNOWNED (SC4001).
  createRoot(() => {
    onSettled(() => {
      onSettled(() => () => {});
    });
  });
  return <i />;
}

export function RootOwnedLeaf() {
  // Control: an onCleanup in an owned onSettled body throws CLEANUP_IN_FORBIDDEN_SCOPE
  // (SC3001), not NO_OWNER_CLEANUP: the leaf's own owner forbids children.
  createRoot(() => {
    onSettled(() => {
      onCleanup(() => {});
    });
  });
  return <i />;
}

export function RootOwnCleanup() {
  // Control: a cleanup the root itself registers is owned, so nothing is reported.
  createRoot(() => {
    onCleanup(() => {});
  });
  return <i />;
}

export function RootNestedMemo() {
  // Control: a memo needs no owner, and the queued callback may create one.
  createRoot(() => {
    onSettled(() => {
      onSettled(() => {
        createMemo(() => count());
      });
    });
  });
  return <i />;
}
