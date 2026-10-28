import { action, createEffect, createMemo, createSignal, onSettled, resolve, until } from "solid-js";

const [ready] = createSignal(false);
// Each helper is called once, from a memo compute (ADR 0494): SC2005 and
// SC2004 are violations there, as in the compute itself.
function waitReady() {
  return until(() => ready());
}
function readNow() {
  return resolve(() => ready());
}

export function Scopes() {
  // Controls: directly in a tracked compute, both are violations.
  const a = createMemo(() => {
    void until(() => ready());
    return ready();
  });
  const b = createMemo(() => resolve(() => ready()));
  const c = createMemo(() => {
    void waitReady();
    return ready();
  });
  const d = createMemo(() => readNow());
  return <div>{String(a())}{String(b())}{String(c())}{String(d())}</div>;
}

// Imperative scopes the rules' hints recommend: no finding.
export const save = action(function* () {
  yield until(() => ready());
});
export function ImperativeScopes() {
  onSettled(() => {
    void until(() => ready());
  });
  createEffect(() => ready(), () => {
    void resolve(() => ready());
  });
  return (
    <button
      onClick={async () => {
        await until(() => ready());
      }}
    >
      go
    </button>
  );
}

// A callback handed to an opaque function: whether it runs under an observer
// is unproven (SC2004 uncertifiable).
declare function invoke(callback: () => unknown): void;
export function OpaqueCallback() {
  invoke(() => resolve(() => ready()));
  return <div />;
}

// Nothing places an exported helper with no caller here: SC2005 and SC2004
// uncertifiable.
export function waitReadyExported() {
  return until(() => ready());
}
export function readNowExported() {
  return resolve(() => ready());
}

// Not a violation (ADR 0493 review): `[1].reduce` with no initial value never
// calls its callback, so a library callback is not crossed.
export function ReduceCallback() {
  const m = createMemo(() =>
    [1].reduce((total) => {
      void resolve(() => ready());
      return total;
    }),
  );
  return <div>{m()}</div>;
}
