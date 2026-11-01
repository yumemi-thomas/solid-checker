import { createEffect, createMemo, createRenderEffect, createSignal } from "solid-js";

export function AliasApply() {
  const [count] = createSignal(0);
  const read = () => {
    count();
  };
  const apply = read;
  createEffect(() => 1, apply);
  return <div />;
}

export function AliasChainRenderApply() {
  const [count] = createSignal(0);
  const read = () => {
    count();
  };
  const first = read;
  const apply = first;
  createRenderEffect(() => 1, apply);
  return <div />;
}

export function AliasCompute() {
  const [count] = createSignal(0);
  const read = () => {
    count();
    return 1;
  };
  const compute = read;
  createEffect(compute, () => {});
  return <div />;
}

export function AliasMemo() {
  const [count] = createSignal(0);
  const read = () => {
    count();
    return 1;
  };
  const compute = read;
  const value = createMemo(compute);
  return <div>{value()}</div>;
}

export function LetSourceApply() {
  const [count] = createSignal(0);
  let read = () => {
    count();
  };
  read = () => {};
  const apply = read;
  createEffect(() => 1, apply);
  return <div />;
}

export function NamedExpressionApply() {
  const [count] = createSignal(0);
  const apply = function step() {
    count();
  };
  createEffect(() => 1, apply);
  return <div />;
}

export function NamedExpressionCompute() {
  const [count] = createSignal(0);
  const compute = function step() {
    count();
    return 1;
  };
  createEffect(compute, () => {});
  return <div />;
}

export function AliasUsedBeforeInitialized() {
  const [count] = createSignal(0);
  const early = () => createEffect(() => 1, apply);
  early();
  const read = () => {
    count();
  };
  const apply = read;
  return <div />;
}
