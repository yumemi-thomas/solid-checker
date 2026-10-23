import { onCleanup } from "solid-js";

// Event-only handlers held by a function-local object literal the checker
// resolves exactly (ADR 0358). The graph edge to the handler is proven unowned
// unless a later attribute or spread may replace the key (ADR 0364).

// Positive control: nothing follows the spread.
export function NoOverride() {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return <button {...holder}>exact holder</button>;
}

// A later `onClick` attribute replaces the key: obligation.
export function LaterAttribute() {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <button {...holder} onClick={() => undefined}>
      later attribute
    </button>
  );
}

// A later exact holder that holds `onClick` replaces it: obligation.
export function LaterSpreadWithKey() {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <button {...holder} {...{ onClick: () => undefined }}>
      later spread with key
    </button>
  );
}

// A later exact holder with only another event type leaves it: violation.
export function LaterSpreadOtherKey() {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <div {...holder} {...{ onScroll: () => undefined }}>
      later spread other key
    </div>
  );
}

// A later spread of unknown keys may hold `onClick`: obligation.
export function LaterSpreadUnknown(props: { rest: { onClick?: () => void } }) {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <button {...holder} {...props.rest}>
      later unknown spread
    </button>
  );
}

// An earlier attribute is the earlier source: the holder's key wins: violation.
export function EarlierAttribute() {
  const holder = {
    onClick: () => {
      onCleanup(() => undefined);
    },
  };
  return (
    <button onClick={() => undefined} {...holder}>
      earlier attribute
    </button>
  );
}
