import { onCleanup, onSettled, untrack, createRoot } from "solid-js";

// Fixture shape with real runtime values. Each helper takes the value that
// decides whether its await runs (skip = fixture's own value; take = await runs).
const pending: Promise<number> = Promise.resolve(1);
let callback: ((value: number) => void) | undefined;
let table: Record<number, number> | undefined;
let flag = false;

async function nullishAssigned(seed: number | undefined) {
  let value = seed;
  value ??= await pending;
  onCleanup(() => {});
  return value;
}
export function NullishSkip() {
  onSettled(() => { void nullishAssigned(1); });
  return <div />;
}
export function NullishTake() {
  onSettled(() => { void nullishAssigned(undefined); });
  return <div />;
}

async function optionalCall(cb: ((value: number) => void) | undefined) {
  cb?.(await pending);
  onCleanup(() => {});
}
export function OptionalCallSkip() {
  onSettled(() => { void optionalCall(callback); });
  return <div />;
}
export function OptionalCallTake() {
  onSettled(() => { void optionalCall(() => {}); });
  return <div />;
}

async function optionalKey(tbl: Record<number, number> | undefined) {
  void tbl?.[await pending];
  onCleanup(() => {});
}
export function OptionalKeySkip() {
  onSettled(() => { void optionalKey(table); });
  return <div />;
}
export function OptionalKeyTake() {
  onSettled(() => { void optionalKey({}); });
  return <div />;
}

async function labelled(f: boolean) {
  skip: {
    if (f) break skip;
    await pending;
  }
  onCleanup(() => {});
}
export function LabelledSkip() {
  onSettled(() => { void labelled(true); });
  return <div />;
}
export function LabelledTake() {
  onSettled(() => { void labelled(flag); });
  return <div />;
}

async function destructuringDefault(input: { value?: number }) {
  const { value = await pending } = input;
  onCleanup(() => {});
  return value;
}
export function DefaultSkip() {
  onSettled(() => { void destructuringDefault({ value: 1 }); });
  return <div />;
}
export function DefaultTake() {
  onSettled(() => { void destructuringDefault({}); });
  return <div />;
}

async function awaitsFirst() {
  await pending;
  onCleanup(() => {});
}
export function UnconditionalAwait() {
  onSettled(() => { void awaitsFirst(); });
  return <div />;
}

async function conditional(flag: boolean) {
  if (flag) await pending;
  onCleanup(() => {});
}
export function ConditionalTake() {
  onSettled(() => { void conditional(true); });
  return <div />;
}
export function ConditionalSkip() {
  onSettled(() => { void conditional(false); });
  return <div />;
}

function registerWrapped() { onCleanup(() => {}); }
async function throughHelper(flag: boolean) {
  if (flag) await pending;
  registerWrapped();
}
export function WrappedTake() {
  onSettled(() => { void throughHelper(true); });
  return <div />;
}
async function throughCallback(flag: boolean) {
  if (flag) await pending;
  untrack(() => onCleanup(() => {}));
}
export function CallbackTake() {
  onSettled(() => { void throughCallback(true); });
  return <div />;
}

async function throughFreshRoot(flag: boolean) {
  if (flag) await pending;
  createRoot(dispose => { onCleanup(() => {}); dispose(); });
}
export function FreshRootControl() {
  onSettled(() => { void throughFreshRoot(true); });
  return <div />;
}
