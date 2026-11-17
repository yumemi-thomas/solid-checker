import { createContext, useContext } from "solid-js";
import { dynamic } from "@solidjs/web";

// A static `dynamic` source that is a promise throws where the call runs (DYNAMIC_STATIC_PROMISE, SC2007, a plain throw); `useContext` of a context
// default-less and never provided: a read under an owner throws ContextNotFoundError, a plain throw with no diagnostic (ADR 0893).
const Bare = createContext<number>();

// ADR 1049: the throw ends where the function runs. An identity wrapper's call is the function itself; a generator stepped once where it is
// made runs its body up to the first `yield` on that `next()` call.
function same(cb: () => void) {
  return cb;
}

// Violations: the component body runs the throw (SC2007 for the returned parameter, SC4002 for the generator).
export function ReturnedCalled() {
  const run = same(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  run();
  return <p />;
}
export function ReturnedCalledDirectly() {
  same(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  })();
  return <p />;
}
export function GeneratorStepped() {
  (function* () {
    useContext(Bare);
    yield 1;
  })().next();
  return <p />;
}

// Obligations (uncertifiable): the function's run is not placed.
let saved: (() => void) | undefined;
function keeps(cb: () => void) {
  saved = cb;
  return cb;
}
export function WrapperAlsoStores() {
  const run = keeps(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  run();
  void saved;
  return <p />;
}
function either(cb: () => void, other: () => void) {
  return Math.random() > 0.5 ? cb : other;
}
export function ConditionalReturn() {
  const run = either(
    () => {
      dynamic(() => Promise.resolve("p" as const), { static: true });
    },
    () => {},
  );
  run();
  return <p />;
}
export function ReturnedCaught() {
  const run = same(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  try {
    run();
  } catch {
    // swallowed
  }
  return <p />;
}
export function GeneratorAfterYield() {
  (function* () {
    yield 1;
    useContext(Bare);
  })().next();
  return <p />;
}
export function GeneratorStored() {
  const steps = (function* () {
    useContext(Bare);
    yield 1;
  })();
  steps.next();
  return <p />;
}
// A returned object literal holding the parameter: a violation since ADR 1058 (the box is a closed literal).
function boxed(cb: () => void) {
  return { cb };
}
export function ReturnedInLiteral() {
  const box = boxed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb();
  return <p />;
}
