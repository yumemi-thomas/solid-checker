import { createEffect, createMemo, createSignal, flush } from "solid-js";

const [n] = createSignal(0);

function* flushes() {
  flush();
  yield 1;
}
function* makes() {
  createMemo(() => 1);
  yield 1;
}
function* delegates() {
  yield* flushes();
}
function* delegatesMake() {
  yield* makes();
}
function consume(steps: () => Generator<number, void, unknown>) {
  for (const _ of steps()) void _;
}
function flushesPlain() {
  flush();
}
function makesPlain() {
  createMemo(() => 1);
}

// A generator's body runs where an iterator of it is stepped: in an effect's apply, a `for…of`, a spread, a `yield*`
// chain and a handed-on generator literal all run it there (ADR 0961). The flush and the creation are uncertifiable
// rows of SC2008 and SC6002, never silence.
export function ForOfFlush() {
  createEffect(() => n(), () => {
    for (const _ of flushes()) void _;
  });
  return <div />;
}
export function SpreadFlush() {
  createEffect(() => n(), () => {
    void [...flushes()];
  });
  return <div />;
}
export function ChainFlush() {
  createEffect(() => n(), () => {
    for (const _ of delegates()) void _;
  });
  return <div />;
}
export function NextFlush() {
  createEffect(() => n(), () => {
    const steps = flushes();
    steps.next();
  });
  return <div />;
}
export function HandedFlush() {
  createEffect(() => n(), () => {
    consume(function* () {
      flush();
      yield 1;
    });
  });
  return <div />;
}
export function ForOfMake() {
  createEffect(() => n(), () => {
    for (const _ of makes()) void _;
  });
  return <div />;
}
export function SpreadMake() {
  createEffect(() => n(), () => {
    void [...makes()];
  });
  return <div />;
}
export function ChainMake() {
  createEffect(() => n(), () => {
    for (const _ of delegatesMake()) void _;
  });
  return <div />;
}

// The plain helpers: the walk's own rows (a violation for the flush, an obligation for the creation).
export function PlainFlush() {
  createEffect(() => n(), () => {
    flushesPlain();
  });
  return <div />;
}
export function PlainMake() {
  createEffect(() => n(), () => {
    makesPlain();
  });
  return <div />;
}

// Silent: a generator called and discarded runs none of its body.
export function Discarded() {
  createEffect(() => n(), () => {
    flushes();
    makes();
  });
  return <div />;
}
// Silent: stepped in the effect's compute function, not its apply (the apply phase is not running there).
export function InCompute() {
  createEffect(
    () => {
      for (const _ of makes()) void _;
      return n();
    },
    () => {},
  );
  return <div />;
}
// Silent: stepped by a handler.
export function InHandler() {
  return (
    <button
      onClick={() => {
        for (const _ of flushes()) void _;
        void [...makes()];
      }}
    >
      go
    </button>
  );
}
