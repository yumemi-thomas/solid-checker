import { action, createEffect, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// Batch 11, slice 5 (the review of #213, B1 and B2): the census class of the code around a flush says where the function around it runs. It is a statement about
// the flush's own stack only when the flush is written in the body of that function, and, for a scheduler's callback, when nothing can enter that function again by
// its name. The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on its class alone.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// B1. A named function expression a step enters again by its name (`tick(true)`): the second run is on the step's stack, where `flush()` throws FLUSH_IN_ACTION (rc.21,
// Chrome; the fuzz cells `fc-named-*`). One uncertifiable row of each code (three rows), never silence.
export function Again() {
  setTimeout(function tick(again?: boolean) {
    flush();
    if (!again) {
      const run = action(function* () {
        tick(true);
      });
      void run();
    }
  });
  return <p>x</p>;
}

// B1, a carrier's named callback in the timer's callback (three rows).
export function Each() {
  setTimeout(() => {
    [1].forEach(function each(again?: unknown) {
      flush();
      if (again !== true) {
        const run = action(function* () {
          each(true);
        });
        void run();
      }
    });
  });
  return <p>x</p>;
}

// B2. A default value runs where its function is called, not where the function around it was written: here an effect's apply (three rows).
let late: undefined | ((x?: void) => void);
export function Default() {
  setTimeout(() => {
    late = (x = flush()) => x;
  });
  createEffect(() => n(), () => {
    late?.();
  });
  return <p>x</p>;
}

// B2 after an `await`: the default is written in the continuation and runs where `later` is called (three rows).
let later: undefined | ((x?: void) => void);
export async function Awaited(ready: Promise<void>) {
  await ready;
  later = (x = flush()) => x;
}

// N1 (the re-review of #213). A class field's initializer runs when an instance is constructed, not where the class is written: here in an effect's apply (three rows).
let Late: undefined | (new () => object);
export function Field() {
  setTimeout(() => {
    Late = class {
      x = flush();
    };
  });
  createEffect(() => n(), () => {
    if (Late) new Late();
  });
  return <p>x</p>;
}

// N2. A generator handed to an inline carrier: `map` only creates the generator object, its body runs at `.next()`, on whatever stack calls it (three rows).
let gens: undefined | Generator<void, void, unknown>[];
export function Carrier() {
  setTimeout(() => {
    gens = [1].map(function* () {
      flush();
    });
  });
  createEffect(() => n(), () => {
    gens?.[0]?.next();
  });
  return <p>x</p>;
}

// N3. A prop written in JSX is a getter: it runs where the component reads `props.v`, here in an effect's apply (three rows).
function Child(props: { v: void }) {
  createEffect(() => n(), () => {
    props.v;
  });
  return <p>c</p>;
}
export function Prop() {
  setTimeout(() => {
    document.body.append((<Child v={flush()} />) as unknown as Node);
  });
  return <p>x</p>;
}

// Silent, each on a typed reason. A named function expression whose name nothing uses can only run as often as the scheduler runs it (`FreshStack`); the flush directly
// in the timer's callback (`FreshStack`); and past the await (`AwaitContinuation`).
export function Once() {
  setTimeout(function tock() {
    flush();
  });
  return <p>x</p>;
}

export function Direct() {
  setTimeout(() => {
    flush();
  });
  return <p>x</p>;
}

export async function After(ready: Promise<void>) {
  await ready;
  flush();
}

// A static initializer runs when the class is evaluated, which is when the timer's callback runs (`FreshStack`).
export function Static() {
  setTimeout(() => {
    class Once {
      static x = flush();
    }
    void Once;
  });
  return <p>x</p>;
}
