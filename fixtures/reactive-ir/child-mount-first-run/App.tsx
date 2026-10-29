import { For, Show, createMemo, createRenderEffect, createSignal } from "solid-js";

// ADR 0507: a component a parent renders as a plain child, the parent itself
// mounted by a module-scope render (main.tsx) and running nothing else, runs
// its body inside the parent's, outside every flush: ADRs 0503-0504's
// first-run claims hold for it.

function WriteChild() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// Violation (SC2001): the render effect's first apply runs on the call.
export function PlainParent() {
  return (
    <div>
      <WriteChild />
    </div>
  );
}

function PendingChild() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      data();
    },
  );
  return <p>x</p>;
}

function Middle() {
  return (
    <section>
      <PendingChild />
    </section>
  );
}

// Violation (SC5001): two parents deep, the first apply reads the pending source.
export function Grand() {
  return (
    <main>
      <Middle />
    </main>
  );
}

function WriteChildB() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(2);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: the parent runs code before rendering the child (a pass
// that went pending would stage the child's).
export function CallingParent() {
  const [flag] = createSignal(true);
  void flag;
  return (
    <div>
      <WriteChildB />
    </div>
  );
}

function WriteChildC() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(3);
    },
  );
  return <p>{count()}</p>;
}

// Violation (SC2001, ADR 0508): `<Show when={true}>` renders its child in its
// value memo, which computes when `Show` is created.
export function ShowParent() {
  return (
    <div>
      <Show when={true}>
        <WriteChildC />
      </Show>
    </div>
  );
}

function WriteChildD(props: { n: number }) {
  const [count, setCount] = createSignal(props.n);
  createRenderEffect(
    () => 1,
    () => {
      setCount(4);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: the tag passes a prop.
export function PropParent() {
  return (
    <div>
      <WriteChildD n={1} />
    </div>
  );
}

function WriteChildE() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(5);
    },
  );
  return <p>{count()}</p>;
}

declare const wide: boolean;

// Uncertifiable: the child is rendered by one of two returns.
export function ConditionalParent() {
  if (wide) {
    return <span />;
  }
  return (
    <div>
      <WriteChildE />
    </div>
  );
}

function WriteChildF() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(6);
    },
  );
  return <p>{count()}</p>;
}

// Violation (SC2001, ADR 0508): a one-row `<For>` renders its row on the
// parent's insert's first pass.
export function ForParent() {
  return (
    <ul>
      <For each={[1]}>{() => <WriteChildF />}</For>
    </ul>
  );
}

function WriteChildG() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(7);
    },
  );
  return <p>{count()}</p>;
}

const shown = true as boolean;

// Uncertifiable: `when` is not the literal `true`.
export function ShowBindingParent() {
  return (
    <div>
      <Show when={shown}>
        <WriteChildG />
      </Show>
    </div>
  );
}

function WriteChildH() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(8);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: an empty list renders no row.
export function ForEmptyParent() {
  return (
    <ul>
      <For each={[] as number[]}>{() => <WriteChildH />}</For>
    </ul>
  );
}

function WriteChildI() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(9);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: the row takes the item.
export function ForItemParent() {
  return (
    <ul>
      <For each={[1]}>{(item) => <WriteChildI />}</For>
    </ul>
  );
}
