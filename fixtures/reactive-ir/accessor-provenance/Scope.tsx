import { createEffect, createMemo, createRenderEffect, createSignal, onSettled } from "solid-js";
import * as Solid from "solid-js";
import { collection } from "./factory";

function memoAccessor() {
  const source = collection(1);
  return createMemo(() => source.rows());
}
export function MemoAccessor() {
  const read = memoAccessor();
  return <p>{read()}</p>;
}

function memoClosure() {
  const source = collection(1);
  return createMemo(() => source.selected());
}
export function MemoClosure() {
  const read = memoClosure();
  return <p>{read()}</p>;
}

function memoHelper() {
  const source = collection(1);
  function read() { return source.rows(); }
  return createMemo(() => read());
}
export function MemoHelper() {
  const read = memoHelper();
  return <p>{read()}</p>;
}

function getterFactory() {
  const [read] = createSignal(1);
  return read;
}
function memoReturned() {
  const read = getterFactory();
  return createMemo(() => read());
}
export function MemoReturned() {
  const read = memoReturned();
  return <p>{read()}</p>;
}

function namespaceMemo() {
  const source = collection(1);
  return Solid.createMemo(() => source.rows());
}
export function NamespaceMemo() {
  const read = namespaceMemo();
  return <p>{read()}</p>;
}

function shadowMemo() {
  const source = collection(1);
  function createMemo(compute: () => number) { return compute(); }
  return createMemo(() => source.rows());
}
export function ShadowMemo() {
  const value = shadowMemo();
  return <p>{value}</p>;
}

function memoArray() {
  const [read] = createSignal(1);
  const objects = [{ read: () => read() }, { read: () => read() + 1 }];
  return createMemo(() => objects.map(item => item.read()));
}
export function MemoArray() {
  const read = memoArray();
  return <p>{read().length}</p>;
}

function effectCompute() {
  const source = collection(1);
  createEffect(() => source.rows(), () => {});
}
export function EffectCompute() {
  effectCompute();
  return <p />;
}

function settledRead() {
  const source = collection(1);
  onSettled(() => { void source.rows(); });
}
export function SettledRead() {
  settledRead();
  return <p />;
}

function eagerRead() {
  const source = collection(1);
  return source.rows();
}
export function EagerRead() {
  const value = eagerRead();
  return <p>{value}</p>;
}

declare function opaque(callback: () => void): void;
function unknownRead() {
  const source = collection(1);
  opaque(() => { void source.rows(); });
}
export function UnknownRead() {
  unknownRead();
  return <p />;
}

function renderApply() {
  const source = collection(1);
  createRenderEffect(() => 1, () => { void source.rows(); });
}
export function RenderApply() {
  renderApply();
  return <p />;
}
