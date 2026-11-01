import { createEffect, createRenderEffect, createSignal } from "solid-js";

function run(callback: () => unknown) {
  callback();
  return 1;
}

export function NamedApplyHelper() {
  const [count] = createSignal(0);
  const apply = () => {
    run(() => {
      count();
    });
  };
  createEffect(() => 1, apply);
  return <div />;
}

export function NamedRenderApplyHelper() {
  const [count] = createSignal(0);
  const apply = () => {
    run(() => {
      count();
    });
  };
  createRenderEffect(() => 1, apply);
  return <div />;
}

export function NamedApplyIife() {
  const [count] = createSignal(0);
  const apply = () => {
    (() => {
      count();
    })();
  };
  createEffect(() => 1, apply);
  return <div />;
}

export function InlineApplyHelper() {
  const [count] = createSignal(0);
  createEffect(() => 1, () => {
    run(() => {
      count();
    });
  });
  return <div />;
}

export function NamedApplyTimer() {
  const [count] = createSignal(0);
  const apply = () => {
    setTimeout(() => {
      count();
    }, 0);
  };
  createEffect(() => 1, apply);
  return <div />;
}

export function NamedApplyStored() {
  const [count] = createSignal(0);
  const later: Array<() => unknown> = [];
  const apply = () => {
    later.push(() => {
      count();
    });
  };
  createEffect(() => 1, apply);
  return <div />;
}

export function NamedComputeHelper() {
  const [count] = createSignal(0);
  const compute = () => {
    run(() => {
      count();
    });
    return 1;
  };
  createEffect(compute, () => {});
  return <div />;
}
