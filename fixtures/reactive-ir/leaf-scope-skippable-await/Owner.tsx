import { onCleanup } from "solid-js";

// `missing-owner` reads the Type Facts producer's after-await calls: a call
// after an await that may be skipped can still run in the component.
declare const pending: Promise<number>;
declare const flag: boolean;

async function registersAfterNullish(seed: number | undefined) {
  let value = seed;
  value ??= await pending;
  onCleanup(() => {});
  return value;
}
export function NullishAwaitInComponent() {
  void registersAfterNullish(1);
  return <div />;
}

async function registersAfterLabel() {
  skip: {
    if (flag) break skip;
    await pending;
  }
  onCleanup(() => {});
}
export function LabelledAwaitInComponent() {
  void registersAfterLabel();
  return <div />;
}

async function registersAfterAwait() {
  await pending;
  onCleanup(() => {});
}
export function UnconditionalAwaitInComponent() {
  void registersAfterAwait();
  return <div />;
}
