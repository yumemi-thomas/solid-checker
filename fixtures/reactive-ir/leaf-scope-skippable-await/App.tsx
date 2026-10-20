import { onCleanup, onSettled } from "solid-js";

// An await that may be skipped does not make what follows a promise
// continuation: when it is skipped, the registration runs in the leaf scope.
declare const flag: boolean;
declare const pending: Promise<number>;
declare const callback: ((value: number) => void) | undefined;
declare const table: Record<number, number> | undefined;

async function nullishAssigned(seed: number | undefined) {
  let value = seed;
  value ??= await pending;
  onCleanup(() => {});
  return value;
}
export function NullishAssignment() {
  onSettled(() => {
    void nullishAssigned(1);
  });
  return <div />;
}

async function optionalCall() {
  callback?.(await pending);
  onCleanup(() => {});
}
export function OptionalCallArgument() {
  onSettled(() => {
    void optionalCall();
  });
  return <div />;
}

async function optionalKey() {
  void table?.[await pending];
  onCleanup(() => {});
}
export function OptionalKey() {
  onSettled(() => {
    void optionalKey();
  });
  return <div />;
}

async function labelled() {
  skip: {
    if (flag) break skip;
    await pending;
  }
  onCleanup(() => {});
}
export function LabelledBreak() {
  onSettled(() => {
    void labelled();
  });
  return <div />;
}

async function destructuringDefault(input: { value?: number }) {
  const { value = await pending } = input;
  onCleanup(() => {});
  return value;
}
export function DestructuringDefault() {
  onSettled(() => {
    void destructuringDefault({ value: 1 });
  });
  return <div />;
}

// Control: an await every run reaches first leaves the registration to a
// later task, `missing-owner`'s question rather than this rule's.
async function awaitsFirst() {
  await pending;
  onCleanup(() => {});
}
export function UnconditionalAwait() {
  onSettled(() => {
    void awaitsFirst();
  });
  return <div />;
}
