import { createEffect, createMemo, createRoot, createSignal } from "solid-js";

// ADR 0536: a helper's read reaching an inline carrier or a callback prop only
// through a call in a named function or a prop literal. rc.17 warns
// STRICT_READ_UNTRACKED on each violation below (full-6 grid cells
// body/root/effect-apply × wrapper × handoff-*, cbp-body × closure/none,
// 2026-11-01).

// Violation: the helper's read runs at `map`, in the body's strict-read window.
export function BodyMap() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  const hf = () => {
    wrapUse();
  };
  [1].map(hf);
  return <p />;
}

// Violation: `forEach` in a root's body.
export function RootForEach() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  createRoot(() => {
    const hf = () => {
      wrapUse();
    };
    [1].forEach(hf);
  });
  return <p />;
}

// Violation: `Function.prototype.call` in an effect apply.
export function ApplyCall() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        wrapUse();
      };
      hf.call(null);
    },
  );
  return <p />;
}

function Kid(props: { run: () => void }) {
  props.run();
  return <p />;
}

// Violation: Kid calls the prop while it renders; the read is the helper's.
export function PropHelper() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  return <Kid run={() => { wrapUse(); }} />;
}

// Violation: the same through a closure written in the prop literal.
export function PropClosure() {
  const [r1] = createSignal(1);
  return (
    <Kid
      run={() => {
        const f = () => {
          r1();
        };
        f();
      }}
    />
  );
}

// Legal: a handler has no strict-read window.
export function HandlerMap() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  const onClick = () => {
    const hf = () => {
      wrapUse();
    };
    [1].map(hf);
  };
  return <p onClick={onClick} />;
}

// Legal: a fresh-stack scheduler runs the function after the root returns.
export function RootTimeout() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  createRoot(() => {
    const hf = () => {
      wrapUse();
    };
    setTimeout(hf);
  });
  return <p />;
}

// Legal (a memo's compute tracks the read), yet uncertifiable at the call as
// a read written in `hf` is: `[1].map` in a compute places no row (ADR 0931).
export function TrackedCompute() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  const m = createMemo(() => {
    const hf = () => {
      wrapUse();
    };
    [1].map(hf);
    return 1;
  });
  return <p>{m()}</p>;
}

function ClickKid(props: { run: () => void }) {
  return <p onClick={() => props.run()} />;
}

// Legal: the consumer calls the prop in a handler.
export function PropHandler() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  return <ClickKid run={() => { wrapUse(); }} />;
}

// Never a violation: a project object's `forEach` is not the library carrier.
export function ProjectCarrier() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  const box = { forEach(_f: () => void) {} };
  const hf = () => {
    wrapUse();
  };
  box.forEach(hf);
  return <p />;
}

function KeepKid(props: { run: () => void }) {
  const kept = props.run;
  void kept;
  return <p />;
}

// Obligation: nothing proves the consumer calls the prop, or when.
export function PropKept() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  return <KeepKid run={() => { wrapUse(); }} />;
}

// Obligation: a function written as a member's value runs wherever the member
// is called, which nothing follows.
export function MemberValue() {
  const [r1] = createSignal(1);
  const wrapUse = () => {
    r1();
  };
  const make = () => {
    const o: { m?: () => void } = {};
    o.m = () => {
      wrapUse();
    };
    return o;
  };
  make().m?.();
  return <p />;
}
