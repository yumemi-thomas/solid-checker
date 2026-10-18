import { createSignal, Show, For, children, createMemo } from "solid-js";
import type { JSX } from "@solidjs/web";

// Direct baselines.
export function DirectShow() {
  const [count] = createSignal(0);
  return <Show when={true}>{(v) => <span>{String(v()) + count()}</span>}</Show>;
}

// Forwarded to a Show function child.
function ShowWrap(props: { render: (x: number) => JSX.Element }) {
  return <Show when={1}>{(v) => props.render(v())}</Show>;
}
export function ViaShow() {
  const [count] = createSignal(0);
  return <ShowWrap render={(x) => { console.log(count()); return <span>{x}</span>; }} />;
}

// Forwarded to Show fallback via a function called in the attribute.
function FallbackWrap(props: { fb: () => JSX.Element }) {
  return <Show when={false} fallback={props.fb()}>x</Show>;
}
export function ViaFallback() {
  const [count] = createSignal(0);
  return <FallbackWrap fb={() => { console.log(count()); return <span /> }} />;
}

// Forwarded to For mapping callback.
function ListWrap(props: { render: (x: number) => JSX.Element }) {
  return <For each={[1, 2]}>{(x) => props.render(x)}</For>;
}
export function ViaFor() {
  const [count] = createSignal(0);
  return <ListWrap render={(x) => { console.log(count()); return <span>{x}</span>; }} />;
}

// children as a function invoked by the component in its body.
function Kids(props: { children: (x: number) => JSX.Element }) {
  const el = props.children(1);
  return <div>{el}</div>;
}
export function ViaChildrenBody() {
  const [count] = createSignal(0);
  return <Kids>{(x) => { console.log(count()); return <span>{x}</span>; }}</Kids>;
}

// children as a function invoked in JSX.
function Kids2(props: { children: (x: number) => JSX.Element }) {
  return <div>{props.children(1)}</div>;
}
export function ViaChildrenJsx() {
  const [count] = createSignal(0);
  return <Kids2>{(x) => { console.log(count()); return <span>{x}</span>; }}</Kids2>;
}

// A render prop called inside a memo returned to JSX.
function MemoWrap(props: { render: () => number }) {
  const m = createMemo(() => props.render());
  return <div>{m()}</div>;
}
export function ViaMemo() {
  const [count] = createSignal(0);
  return <MemoWrap render={() => count()} />;
}

// children() helper.
function Resolved(props: { children: JSX.Element }) {
  const c = children(() => props.children);
  return <div>{c()}</div>;
}
export function ViaChildrenHelper() {
  const [count] = createSignal(0);
  const make = () => { console.log(count()); return <span /> };
  return <Resolved>{make()}</Resolved>;
}

// Render prop invoked in a component attribute of a component that reads it in its body.
function Reader(props: { value: number }) {
  const v = props.value;
  return <span>{v}</span>;
}
function AttrWrap(props: { compute: () => number }) {
  return <Reader value={props.compute()} />;
}
export function ViaComponentAttr() {
  const [count] = createSignal(0);
  return <AttrWrap compute={() => count()} />;
}
