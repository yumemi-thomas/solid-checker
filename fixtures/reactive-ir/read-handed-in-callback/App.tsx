import { createEffect, createRoot, createSignal } from "solid-js";

// ADR 0535: a read in a named function handed by name to an inline carrier
// (ADR 0531) inside a primitive's callback runs at the carrier, in that
// callback's strict-read window. rc.17 warns STRICT_READ_UNTRACKED in a root's
// body and an effect apply (both run under `untrack`, the window open; full-6
// grid cells root/effect-apply × handoff-*, 2026-11-01).

// Violation: `forEach` in a root's body.
export function RootForEach() {
  const [r1] = createSignal(1);
  createRoot(() => {
    const f = () => {
      r1();
    };
    [1].forEach(f);
  });
  return <p />;
}

// Violation: `Function.prototype.call` in an effect apply.
export function ApplyCall() {
  const [r1] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      const f = () => {
        r1();
      };
      f.call(null);
    },
  );
  return <p />;
}

// Legal: a handler has no strict-read window.
export function HandlerForEach() {
  const [r1] = createSignal(1);
  const onClick = () => {
    const f = () => {
      r1();
    };
    [1].forEach(f);
  };
  return <p onClick={onClick} />;
}

// Legal: a fresh-stack scheduler runs the function after the root returns.
export function RootTimeout() {
  const [r1] = createSignal(1);
  createRoot(() => {
    const f = () => {
      r1();
    };
    setTimeout(f);
  });
  return <p />;
}

// Never a violation: a project object's `forEach` is not the library carrier.
export function RootProjectCarrier() {
  const [r1] = createSignal(1);
  const box = { forEach(_f: () => void) {} };
  createRoot(() => {
    const f = () => {
      r1();
    };
    box.forEach(f);
  });
  return <p />;
}

// Obligation: a function written as a member's value runs wherever the member
// is called, which nothing follows (`make().m?.()`).
export function ReturnedMember() {
  const [r1] = createSignal(1);
  const make = () => {
    const o: { m?: () => void } = {};
    o.m = () => {
      r1();
    };
    return o;
  };
  make().m?.();
  return <p />;
}

// Obligation: the same through `this` in a class built in a root's body.
export function RootThisMember() {
  const [r1] = createSignal(1);
  createRoot(() => {
    class Holder {
      m?: () => void;
      constructor() {
        this.m = () => {
          r1();
        };
      }
    }
    new Holder().m?.();
  });
  return <p />;
}
