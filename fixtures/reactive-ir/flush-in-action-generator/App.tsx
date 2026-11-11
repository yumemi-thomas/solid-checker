import { action, flush } from "solid-js";

// A generator literal handed to a helper that makes the action: the generator is the action's step body (ADR 0961).
function useSave(gen: () => Generator<unknown, void, unknown>) {
  return action(gen);
}
const viaHelper = useSave(function* () {
  flush();
  yield Promise.resolve();
});

// `yield*` into a generator whose body flushes.
function* inner() {
  flush();
  yield 1;
}
const viaDelegate = action(function* () {
  yield* inner();
  yield Promise.resolve();
});

// The generator written as the action's own argument: the walk's proof (a violation).
const direct = action(function* () {
  flush();
  yield Promise.resolve();
});

// A generator that does not flush: silent.
function* quiet() {
  yield 1;
}
const viaQuiet = action(function* () {
  yield* quiet();
  yield Promise.resolve();
});

// ADR 0961 (R1): a generator whose iterator an action's step keeps may be stepped anywhere: `Array.from`, `new Set`,
// `.next()` on the call or on a stored iterator, a loop over a stored iterator. No walk follows the iterator, so the
// flush is an uncertifiable row (SC2006), never silence. Chrome fires FLUSH_IN_ACTION at each one when the action starts.
function* fromBody() {
  flush();
  yield 1;
}
const viaFrom = action(function* () {
  void Array.from(fromBody());
  yield Promise.resolve();
});
function* setBody() {
  flush();
  yield 1;
}
const viaSet = action(function* () {
  void new Set(setBody());
  yield Promise.resolve();
});
function* directBody() {
  flush();
  yield 1;
}
const viaDirect = action(function* () {
  directBody().next();
  yield Promise.resolve();
});
function* storedBody() {
  flush();
  yield 1;
}
const viaStored = action(function* () {
  const steps = storedBody();
  steps.next();
  yield Promise.resolve();
});
function* loopBody() {
  flush();
  yield 1;
}
const viaLoop = action(function* () {
  const steps = loopBody();
  for (const _ of steps) void _;
  yield Promise.resolve();
});

// Silent: a generator an action calls and discards runs none of its body, and nothing else steps it.
function* discardedBody() {
  flush();
  yield 1;
}
const viaDiscard = action(function* () {
  discardedBody();
  yield Promise.resolve();
});

// `yield*` two levels deep, and a generator handed to the helper by name: no walk follows either, so each flush is an uncertifiable row.
function* innermost() {
  flush();
  yield 1;
}
function* middle() {
  yield* innermost();
}
const viaChain = action(function* () {
  yield* middle();
  yield Promise.resolve();
});
function* namedBody() {
  flush();
  yield Promise.resolve();
}
const viaNamed = useSave(namedBody);

export function App() {
  return (
    <button
      onClick={() => {
        void viaChain();
        void viaNamed();
        void viaHelper();
        void viaDelegate();
        void direct();
        void viaQuiet();
        void viaFrom();
        void viaSet();
        void viaDirect();
        void viaStored();
        void viaLoop();
        void viaDiscard();
      }}
    >
      go
    </button>
  );
}
