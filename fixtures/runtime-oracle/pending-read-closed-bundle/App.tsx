import { createMemo, createTrackedEffect, onSettled } from "solid-js";

// ADR 1009: a function that is a plain property of a `const` object literal nothing else reaches, handed to a primitive as
// `bundle.key`, runs where that primitive runs its callback.

// Violations: SC5001, a pending read in a leaf owner (`onSettled`, `createTrackedEffect`).
export function Settled() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); } };
  onSettled(bundle.run);
  return <p />;
}

export function Tracked() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); } };
  createTrackedEffect(bundle.run);
  return <p />;
}

export function NestedClosure() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => {
    const inner = () => { user(); };
    inner();
  } };
  onSettled(bundle.run);
  return <p />;
}

export function NestedLiteral() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { (() => { user(); })(); } };
  onSettled(bundle.run);
  return <p />;
}

export function TwoProperties() {
  const user = createMemo(async () => 1);
  const bundle = { run: () => { user(); }, label: "x" };
  onSettled(bundle.run);
  return <p />;
}

// Obligations: something else can reach or change the object, or the property is not a plain data property. Each case has a
// key of its own: a write of a key anywhere in the file opens every member of that name.
function keep(value: unknown) {
  void value;
}

export function ObjectPassed() {
  const user = createMemo(async () => 1);
  const bundle = { passed: () => { user(); } };
  keep(bundle);
  onSettled(bundle.passed);
  return <p />;
}

export function PropertyWritten() {
  const user = createMemo(async () => 1);
  const bundle = { written: () => { user(); } };
  bundle.written = () => {};
  onSettled(bundle.written);
  return <p />;
}

export function ComputedKey() {
  const user = createMemo(async () => 1);
  const key = "computed" as const;
  const bundle = { [key]: () => { user(); } };
  onSettled(bundle[key]);
  return <p />;
}

export function LetObject() {
  const user = createMemo(async () => 1);
  let bundle = { loose: () => { user(); } };
  onSettled(bundle.loose);
  return <p />;
}

export function Method() {
  const user = createMemo(async () => 1);
  const bundle = { method() { user(); } };
  onSettled(bundle.method);
  return <p />;
}

export function MemberAlsoCalled() {
  const user = createMemo(async () => 1);
  const bundle = { called: () => { user(); } };
  bundle.called();
  onSettled(bundle.called);
  return <p />;
}

export function MemberAliased() {
  const user = createMemo(async () => 1);
  const bundle = { aliased: () => { user(); } };
  const held = bundle.aliased;
  onSettled(held);
  return <p />;
}

export function NestedBundle() {
  const user = createMemo(async () => 1);
  const bundle = { inner: { deep: () => { user(); } } };
  onSettled(bundle.inner.deep);
  return <p />;
}

// No violation: the same bundle handed to a tracked compute, which may suspend.
export function TrackedCompute() {
  const user = createMemo(async () => 1);
  const bundle = { compute: () => user() };
  const doubled = createMemo(bundle.compute);
  return <p>{doubled()}</p>;
}
