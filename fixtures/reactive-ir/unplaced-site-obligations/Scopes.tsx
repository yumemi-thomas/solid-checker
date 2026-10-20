import { action, createEffect, createMemo, createSignal, onSettled, resolve, until } from "solid-js";

const [ready] = createSignal(false);
// Nothing places these helpers' calls: SC2005 and SC2004 uncertifiable.
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
