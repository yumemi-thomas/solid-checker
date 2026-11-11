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

export function App() {
  return (
    <button
      onClick={() => {
        void viaHelper();
        void viaDelegate();
        void direct();
        void viaQuiet();
      }}
    >
      go
    </button>
  );
}
