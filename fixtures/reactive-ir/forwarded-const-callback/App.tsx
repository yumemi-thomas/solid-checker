import { createEffect, createSignal } from "solid-js";

// ADR 0931: `hc1` hands its parameter to the `const` arrow `hc0`, which calls
// it on every completion, so `hc1` calls it too (ADR 0285's forward, which
// took only `function` declarations): `hc0`'s declarator runs before `hc1`
// can exist.

// Violation at `dir1()`: the ref expression runs in the body, and `dir1`
// runs the callback through both forwarders (was certified clean).
export function ForwardedInRefFactory() {
  const [count] = createSignal(3);
  const dir1 = () => {
    const hc0 = (g: () => void) => { g(); };
    const hc1 = (g: () => void) => { hc0(g); };
    hc1(() => {
      count();
    });
    return () => {};
  };
  return <p ref={dir1()}>x</p>;
}

// Violation at `dir1()`: module-level forwarders (was certified clean).
const hm0 = (g: () => void) => { g(); };
const hm1 = (g: () => void) => { hm0(g); };
export function ForwardedAtModule() {
  const [count] = createSignal(3);
  const dir1 = () => {
    hm1(() => {
      count();
    });
  };
  dir1();
  return <div>x</div>;
}

// Silent: the forwarding runs in a handler.
export function ForwardedInHandler() {
  const [count] = createSignal(3);
  return (
    <button
      onClick={() => {
        hm1(() => {
          count();
        });
      }}
    >
      go
    </button>
  );
}

// Not proven: `run` is a hoisted declaration, called before `hl0` is
// initialized (it throws in the dead zone, no read runs).
export function HoistedForwarder() {
  const [count] = createSignal(3);
  run(() => {
    count();
  });
  const hl0 = (g: () => void) => { g(); };
  function run(g: () => void) { hl0(g); }
  return <div>x</div>;
}

// Not proven: the forward is on some completions only.
const on = Math.random() > 2;
export function ConditionalForward() {
  const [count] = createSignal(3);
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { if (on) hc0(g); };
  hc1(() => {
    count();
  });
  return <div>x</div>;
}

// Uncertifiable at `wrapUse()`: the apply calls the compute's returned
// function, which no caller edge reaches (was certified clean).
export function ComputeReturnedCall() {
  const [count] = createSignal(3);
  const wrapUse = () => { void count(); };
  createEffect(() => () => { wrapUse(); }, (v) => { v(); });
  return <div>x</div>;
}

// Uncertifiable too, as a read written in the returned function is: the
// role arm does not prove this apply idle.
export function ComputeReturnedIdle() {
  const [count] = createSignal(3);
  const wrapUse = () => { void count(); };
  createEffect(() => () => { wrapUse(); }, (v) => { void v; });
  return <div>x</div>;
}

// Not proven: a default that may throw runs before the body, so `g` may
// never run (`stop()` always throws). A literal default proves as before.
function stop(): number {
  throw new Error("skip");
}
const gated = (g: () => void, _x = stop()) => { g(); };
const literal = (g: () => void, _x = 0) => { g(); };
export function GatedDefault() {
  const [count] = createSignal(3);
  try { gated(() => { count(); }); } catch { /* skipped */ }
  return <div>x</div>;
}
export function LiteralDefault() {
  const [count] = createSignal(3);
  literal(() => { count(); });
  return <div>x</div>;
}
