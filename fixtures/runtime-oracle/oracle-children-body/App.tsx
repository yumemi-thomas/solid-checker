import { createSignal, For } from "solid-js";
import type { JSX } from "@solidjs/web";

function Kids(props: { children: (x: number) => JSX.Element }) {
  const el = props.children(1);
  return <div>{el}</div>;
}

export function ViaChildrenBody() {
  const [count] = createSignal(0);
  return (
    <Kids>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Kids>
  );
}

export function ViaChildrenAttr() {
  const [count] = createSignal(0);
  return (
    <Kids
      children={(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    />
  );
}

function Opaque(props: { children: (x: number) => JSX.Element }) {
  return <div>{(globalThis as any).x?.(props.children)}</div>;
}

export function ViaOpaque() {
  const [count] = createSignal(0);
  return (
    <Opaque>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Opaque>
  );
}

function InJsx(props: { children: (x: number) => JSX.Element }) {
  return <div>{props.children(1)}</div>;
}

export function TrackedChild() {
  const [count] = createSignal(0);
  return <InJsx>{(x) => <span>{x + count()}</span>}</InJsx>;
}

export function TrackedChildRead() {
  const [count] = createSignal(0);
  return (
    <InJsx>
      {(x) => {
        const n = count();
        return <span>{x + n}</span>;
      }}
    </InJsx>
  );
}

function OnClick(props: { children: () => void }) {
  return <button onClick={() => props.children()}>x</button>;
}

export function EventChild() {
  const [count] = createSignal(0);
  return <OnClick>{() => console.log(count())}</OnClick>;
}

export function ListChild() {
  const [count] = createSignal(0);
  return <For each={[1, 2]}>{(x) => <span>{x + count()}</span>}</For>;
}
