import { createRoot, createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";

function handlers(cb: () => void) {
  return { onClick: cb };
}

// The callback is run through `runWithOwner` (owned) and as the click handler
// (no owner): the handler run is a proven SC4001 violation.
export function RunWithOwnerAndHandler() {
  const owner = getOwner();
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  runWithOwner(owner, h.onClick);
  return <button {...h}>mix</button>;
}

function doubleCall(cb: () => void) {
  const run = () => cb();
  run();
  return { onClick: run };
}

// `run()` calls the callback during owned setup and `run` is also the click
// handler.
export function OwnedRunAndHandler() {
  const h = doubleCall(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}

// `cb` is called in the component body and is the element's handler.
export function PlainCallAndHandler() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  cb();
  return <button onClick={cb}>both</button>;
}

// The same through an exact spread holder.
export function PlainCallAndSpreadHolder() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  const holder = { onClick: cb };
  cb();
  return <button {...holder}>both</button>;
}

function sharedCleanup() {
  onCleanup(() => undefined);
}

// The helper is reached from the owned setup and from the click handler: its
// `onCleanup` is a violation of the click run, named by the event only.
export function HelperOnEventPath() {
  const cb = () => {
    sharedCleanup();
  };
  cb();
  const h = handlers(cb);
  return <button {...h}>helper</button>;
}

function outer(cb: () => void) {
  return createRoot(() => ({ onClick: cb }));
}

// The callback only passes through the root callback's return value: the root
// never invokes it, the click handler does, with no owner.
export function ClosureRoot() {
  const h = outer(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>unowned despite closure</button>;
}

// Later definitions replace the key (ADR 0364): the compiler lowers an element
// with a spread to `spread(el, [sources...])` in source order and the runtime
// keeps a key from the last source that has it, so the callback is not the
// handler and its only run is the owned one. Obligations, not violations.
export function LaterAttributeOverride() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  const h = { onClick: cb };
  return (
    <button {...h} onClick={() => undefined}>
      later attribute
    </button>
  );
}

// A later spread whose keys are unknown may carry `onClick`.
export function LaterSpreadUnknown(props: { rest: { onClick?: () => void } }) {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  const h = { onClick: cb };
  return (
    <button {...h} {...props.rest}>
      later spread
    </button>
  );
}

// A later spread of an exact holder that holds the key replaces it.
export function LaterSpreadExactWithKey() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  const h = { onClick: cb };
  return (
    <button {...h} {...{ onClick: () => undefined }}>
      later exact key
    </button>
  );
}

// Positive. A later spread of an exact holder with no `onClick` (only another
// event type) leaves the key alone: still a violation.
export function LaterSpreadExactWithoutKey() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  const h = { onClick: cb };
  return (
    <div {...h} {...{ onScroll: () => undefined }}>
      later exact other key
    </div>
  );
}

// Positive. An attribute BEFORE the spread is the earlier source: the holder's
// key wins, so the callback is the handler.
export function AttributeBeforeSpread() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  const h = { onClick: cb };
  return (
    <button onClick={() => undefined} {...h}>
      attribute before spread
    </button>
  );
}

function Forwarded(props: { onClick?: () => void; children?: unknown }) {
  return (
    <button {...props} onClick={() => undefined}>
      {props.children}
    </button>
  );
}

// A component spreads its props onto a `<button>` and then sets its own
// `onClick`: the prop it was given is replaced.
export function ComponentSpreadOverride() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  return <Forwarded onClick={cb}>forwarded</Forwarded>;
}

// The event-only analogue: the holder's callback has no other invocation, and
// the later attribute may leave it no run at all.
export function EventOnlyOverride() {
  const h = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <button {...h} onClick={() => undefined}>
      event only
    </button>
  );
}

// The compiler's own `EventHandler` role (ADR 0364): `onClick={cb}` is lowered
// into the same sources array, so a later spread that holds the key replaces it.
export function AttributeThenSpread() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  return (
    <button onClick={cb} {...{ onClick: () => undefined }}>
      attribute then spread
    </button>
  );
}

// Positive. The same attribute with a later exact holder that has no `onClick`.
export function AttributeThenSpreadWithoutKey() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(getOwner(), cb);
  return (
    <div onClick={cb} {...{ onScroll: () => undefined }}>
      attribute then other key
    </div>
  );
}

// An inline handler literal followed by a spread whose keys are unknown.
export function InlineThenUnknownSpread(props: { rest: { onClick?: () => void } }) {
  return (
    <button
      onClick={() => {
        onCleanup(() => undefined);
      }}
      {...props.rest}
    >
      inline then spread
    </button>
  );
}

// Negative. The callback is invoked through `runWithOwner` and nowhere else:
// no event edge, so no violation.
export function RunWithOwnerOnly() {
  const owner = getOwner();
  const cb = () => {
    onCleanup(() => undefined);
  };
  runWithOwner(owner, cb);
  return <p>owner only</p>;
}

const registry: { onClick?: () => void } = {};

// Negative. The event invocation goes through a mutable holder the walk cannot
// follow: whether the click handler is this callback is unproven.
export function UnresolvedHolder() {
  const cb = () => {
    onCleanup(() => undefined);
  };
  registry.onClick = cb;
  runWithOwner(getOwner(), cb);
  return <button {...registry}>unresolved</button>;
}

// Negative. A write is a different question: the same callback runs owned on
// the other path, where the write is not a defect, so it stays an obligation.
export function WriteInMixedCallback() {
  const [, setCount] = createSignal(0);
  const h = handlers(() => {
    setCount(1);
  });
  runWithOwner(getOwner(), h.onClick);
  return <button {...h}>write</button>;
}

// Negative. The `onCleanup` is inside a root the mixed callback creates: the
// root owns it on every run, the event run included.
export function RootInsideMixedCallback() {
  const h = handlers(() => {
    createRoot(() => {
      onCleanup(() => undefined);
    });
  });
  runWithOwner(getOwner(), h.onClick);
  return <button {...h}>root</button>;
}

// Negative. The owner probe guards the cleanup on every run of the callback, so
// the event run never reaches it.
export function GuardedCleanup() {
  const h = handlers(() => {
    if (getOwner()) onCleanup(() => undefined);
  });
  runWithOwner(getOwner(), h.onClick);
  return <button {...h}>guarded</button>;
}
