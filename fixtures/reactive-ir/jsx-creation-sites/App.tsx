import { createEffect, createSignal, For, Match, onSettled, Repeat, Show, Switch } from "solid-js";

const [count] = createSignal(0);
const rest = { class: "x" };
const fn: any = () => 1;
function shared() {
  return <div>{count()}</div>;
}
const text = "t" as string;
function Comp(props: { children?: unknown }) {
  return <i>{props.children as any}</i>;
}

// Proved: no owner at a render effect.
export const atModule = <p>{count()}</p>;
export function Timer() {
  setTimeout(() => {
    void (<p>{count()}</p>);
    void (<p class={String(count())} />);
    void (<div {...rest} />);
  }, 0);
  return <i />;
}
export async function afterAwait() {
  await Promise.resolve();
  return <p>{count()}</p>;
}

// Proved: an apply's stack and a leaf hold no owner to create under.
export function Phases() {
  createEffect(
    () => 1,
    () => {
      void (<p>{count()}</p>);
      void (<Comp />);
      void (<>{count()}</>);
    },
  );
  onSettled(() => {
    void (<p>{count()}</p>);
    void (<Comp />);
    void (<>{count()}</>);
    (() => {
      void (<Comp />);
    })();
  });
  return <i />;
}

// Uncertifiable: a value that may hold a function, and a getter read wherever
// the component reads it.
export function Open() {
  setTimeout(() => {
    void (<p>{fn}</p>);
    void (<p>{[fn]}</p>);
    void (<Comp><p>{count()}</p></Comp>);
  }, 0);
  return <i />;
}

// Silent: static JSX, a component element and a fragment child with no owner,
// a child that cannot hold a function, and every site under an owner.
export function Quiet() {
  setTimeout(() => {
    void (<p>static</p>);
    void (<p>{text}</p>);
    void (<Comp />);
    void (<>{count()}</>);
  }, 0);
  return <p class={String(count())}>{count()}</p>;
}

// Silent: a control-flow tag runs its function child under an owner of its own.
export function Flow() {
  return (
    <>
      <For each={[1]}>{(x) => <p>{x + count()}</p>}</For>
      <For each={[1]}>
        {async (x) => {
          await Promise.resolve();
          return <p>{x + count()}</p>;
        }}
      </For>
      <Repeat count={1}>{(i) => <p>{i + count()}</p>}</Repeat>
      <Show when={count()}>{(v) => <p>{(v as () => number)()}</p>}</Show>
      <Switch>
        <Match when={count()}>{(v) => <p>{(v as () => number)()}</p>}</Match>
      </Switch>
    </>
  );
}

// Uncertifiable: a named child keeps its other entries; `length` 0 (a `this:`
// parameter or a default) hands the function back as a value; a component's own
// function child is its to run; a `Match` outside `Switch` is not run; and a local
// `For` is no control-flow tag.
export function FlowOpen() {
  const For2 = (props: { children: () => unknown }) => {
    setTimeout(props.children, 0);
    return <i />;
  };
  const row = (x: number) => <p>{x + count()}</p>;
  return (
    <>
      <For each={[1]}>{row}</For>
      <Show when={count()}>{function (this: unknown) { return <p>{count()}</p>; }}</Show>
      <Show when={1}>
        <Comp>{(x: number) => <p>{x + count()}</p>}</Comp>
      </Show>
      <Show when={count()}>{(v = 1) => <p>{v + count()}</p>}</Show>
      <Match when={count()}>{(v) => <p>{(v as () => number)()}</p>}</Match>
      <For2>{() => <p>{count()}</p>}</For2>
    </>
  );
}

// A helper run from an apply and from a timer: the timer's run has no apply window,
// so both rows are open. Starting like a literal proves nothing.
export function Shared() {
  createEffect(
    () => 1,
    () => {
      shared();
    },
  );
  setTimeout(shared, 0);
  setTimeout(() => {
    void (<p>{1 && fn}</p>);
  }, 0);
  return <i />;
}

