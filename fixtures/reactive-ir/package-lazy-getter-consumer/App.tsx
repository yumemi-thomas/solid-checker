import { createMemo, createEffect, untrack } from "solid-js";
import { makeSize, makeStatic } from "reactive-package";
import * as pkg from "reactive-package";

export function TrackedJsx() {
  const size = makeSize();
  return <div>{size.width}</div>;
}
export function MemoRead() {
  const size = makeSize();
  const width = createMemo(() => size.width);
  createEffect(() => size.height, () => {});
  return <div>{width()}</div>;
}
export function UnprimedRead() {
  const size = makeSize();
  const width = size.width;
  return <div>{width}</div>;
}
export function PrimedRead() {
  const size = makeSize();
  createMemo(() => size.width);
  const width = size.width;
  return <div>{width}</div>;
}
export function StaticLiteral() {
  const [state] = makeStatic({ count: 0 });
  return <div>{state.count}</div>;
}
export function Escaped() {
  const size = makeSize();
  const box = [size];
  void box;
  return <div>{size.width}</div>;
}
export function TransparentReceiver() {
  const size = makeSize();
  return <div>{(size as typeof size).width}</div>;
}
export function NamespaceRead() {
  const size = pkg.makeSize();
  return <div>{size.width}</div>;
}
export function ShadowedFactory() {
  const makeSize = () => ({ width: 1 });
  const size = makeSize();
  return <div>{size.width}</div>;
}
export function GenericInput<T extends Record<string, unknown>>(init: T) {
  const [state] = makeStatic(init);
  return <div>{String(state.count)}</div>;
}
export function ComputedKey() {
  const size = makeSize();
  return <div>{size["width"]}</div>;
}
export function MutatedBinding() {
  let size = makeSize();
  return <div>{size.width}</div>;
}
export function StaticFunctionValue() {
  const [state] = makeStatic({ count: () => 0 });
  return <div>{String(state.count)}</div>;
}
export function RetainedSetter() {
  const [state, setState] = makeStatic({ count: 0 });
  void setState;
  return <div>{state.count}</div>;
}
export function ClearedUntrack() {
  const size = makeSize();
  const width = untrack(() => size.width);
  return <div>{width}</div>;
}
export function DeferredAlias() {
  const size = makeSize();
  const width = () => size.width;
  return <div>{width()}</div>;
}
export function wrapSize() {
  return makeSize();
}
export function WrapperOpen() {
  const size = wrapSize();
  return <div>{size.width}</div>;
}
