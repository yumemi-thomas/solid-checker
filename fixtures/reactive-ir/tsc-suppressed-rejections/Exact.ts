import { createEffect, createSignal, onSettled } from "solid-js";

const [count] = createSignal(0);

// ADR 0830: a rejection both compilers suppress on the diagnostic's own line.
export function exact() {
  // @ts-ignore
  createEffect(() => count());
  // @ts-expect-error
  createEffect(() => count(), 5);
  /// @ts-ignore
  createEffect(() => count(), null);
}

// The directive covers another line: TS2554 stands, the call is TypeScript's.
export function elsewhere() {
  // @ts-ignore
  const unused: number = "a";
  createEffect(() => count());
  return unused;
}

// A cleanup the published type rejects, on a suppressed line: uncertifiable.
export function returns() {
  createEffect(() => count(), (value) => {
    // @ts-ignore
    return value + 1;
  });
  // @ts-ignore
  onSettled(() => 1);
  // A function literal is a cleanup: nothing open.
  createEffect(() => count(), () => {
    // @ts-ignore
    return () => {};
  });
}
