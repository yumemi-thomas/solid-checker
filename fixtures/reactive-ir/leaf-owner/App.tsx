import { createEffect, createMemo, createRoot, createSignal, createTrackedEffect, flush, onCleanup, onSettled } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  createMemo(() => count());

  onSettled(() => {
    onCleanup(() => {});
    createSignal(1);
    createSignal(() => count());
    flush();
  });

  createTrackedEffect(() => {
    onCleanup(() => {});
    createMemo(() => count());
    createRoot(() => {});
    flush();
  });

  onSettled(() => {
    console.log("settled");
    onCleanup(() => console.log("disposed"));
  });

  onSettled(() => 42);
  createTrackedEffect(() => "invalid");
  createEffect(() => count(), () => ({ invalid: true }));
  onSettled(async () => {});
  createEffect(() => count(), async () => {});

  onSettled(() => undefined);
  createTrackedEffect(() => () => console.log("valid"));
  onSettled(() => {
    if (count()) return 99;
    return () => console.log("valid branch");
  });
  const cleanup = () => console.log("cleanup");
  onSettled(() => {
    return cleanup;
  });

  return <div>{count()}</div>;
}

// Out-of-band onSettled: called from an event handler, the callback is
// enqueued as a plain function, not a leaf owner (rc.0 dev.js:4855-4893) —
// onCleanup only warns missing-owner,    primitives attach nowhere without
// throwing, and flush() is a silent no-op, so no SC3xxx fires here.
export function OutOfBand() {
  const [count] = createSignal(0);
  return (
    <button
      onClick={() =>
        onSettled(() => {
          onCleanup(() => {});
          createMemo(() => count());
          flush();
        })
      }
    >
      {count()}
    </button>
  );
}

// An exported helper's callers are unknowable: owner-backed the onCleanup
// throws, out-of-band it only warns — so the leaf-owner finding is an
// uncertifiable proof obligation, not a proven violation.
export function settleWithCleanup() {
  onSettled(() => {
    onCleanup(() => {});
  });
}

// Dynamic extent: these helpers perform forbidden operations in their own
// synchronous extent, so calling one from a leaf callback throws exactly
// like the inline spelling. The findings anchor at the call site inside the
// leaf scope — the helper body has other, legal callers.
function registerTeardown() {
  onCleanup(() => {});
}

function flushNow() {
  flush();
}

// Transitive: one exact hop deeper still executes synchronously.
function indirectTeardown() {
  registerTeardown();
}

// A helper whose synchronous extent creates an owner-attaching primitive:
// inside a leaf scope there is no owner to attach it to.
function trackDouble() {
  createMemo(() => 2);
}

// The forbidden call sits inside a nested function the helper only builds;
// calling the helper executes nothing forbidden.
function buildTeardownHandler() {
  const handler = () => onCleanup(() => {});
  return handler;
}

export function DynamicExtent() {
  const [count] = createSignal(0);
  createTrackedEffect(() => {
    count();
    registerTeardown(); // SC3001, via registerTeardown()
    flushNow(); // SC3003, via flushNow()
    indirectTeardown(); // SC3001, via indirectTeardown()
    trackDouble(); // SC3002, via trackDouble()
    buildTeardownHandler(); // clean: nested body is not synchronous extent
  });
  // The same synchronous extent written without braces: an expression-bodied
  // leaf callback is still the callback the owner receives. (What it returns
  // is `flushNow()`'s proven void, so it registers no returned cleanup.)
  createTrackedEffect(() => flushNow()); // SC3003, via flushNow()
  return (
    <button
      onClick={() => {
        // Not a leaf scope: an event handler runs out-of-band, where
        // onCleanup at worst warns missing-owner    — the leaf rules stay
        // silent on this call.
        registerTeardown();
      }}
    >
      {count()}
    </button>
  );
}

// Argument position: the dynamic-extent reasoning only applies to calls in
// the *leaf callback's own* synchronous extent, and a call argument that is
// not a function literal is not that callback. Both of these stay clean.
function makeTeardownCallback() {
  return () => {
    registerTeardown();
  };
}

function wrapCallback(callback: () => void) {
  return callback;
}

export function ArgumentPosition() {
  // `makeTeardownCallback()` is evaluated here, at argument-evaluation time,
  // under the enclosing owner — before any leaf scope exists. The callback
  // the owner ends up with is opaque. No SC3xxx is proven, but SC9012 keeps
  // the unresolved leaf behavior explicit.
  createTrackedEffect(makeTeardownCallback());
  // The arrow is `wrapCallback`'s argument, not the owner's callback:
  // `wrapCallback` decides whether and where it runs. Calls written inside
  // it are not proven to execute in the leaf scope, so SC9012 is emitted
  // instead of guessing at SC3001 or silently certifying the callback.
  createTrackedEffect(
    wrapCallback(() => {
      registerTeardown();
    }),
  );
  return <div />;
}

const exactLeafCleanup = () => {
  onCleanup(() => {});
};
const exactLeafSafe = () => {
  console.log("safe callback");
};

export function ExactCallbackReference() {
  // Exact in-project callback references are inspectable: the first is a
  // proven SC3001 violation, while the second is certified and must not be
  // widened to SC9012 merely because it is not written inline.
  createTrackedEffect(exactLeafCleanup);
  createTrackedEffect(exactLeafSafe);
  return <div />;
}
