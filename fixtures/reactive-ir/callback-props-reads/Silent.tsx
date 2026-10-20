import { createSignal, createStore } from "solid-js";

// ADR 0323 refusal controls: callbacks that reach none of the modelled
// consumers keep the silence a props or store read had before the ADR. The
// signal twin of each one *is* reported (an uncertifiable "passed to a
// consumer not proven to invoke it"); closing that gap for props and store
// reads is an owner decision recorded in the ADR.

function Button(props: { onPick: () => void }) {
  return <div onClick={() => props.onPick()} />;
}

// A callback on a component attribute.
function OnAttribute(props: { x: number }) {
  return <Button onPick={() => console.log(props.x)} />;
}
// An unknown callee (declared in opaque.d.ts, no body).
function UnknownCallee(props: { x: number }) {
  opaqueCb(() => console.log(props.x));
  return <div />;
}
// A standard-library callee that may keep or schedule the callback.
function Timer(props: { x: number }) {
  setTimeout(() => console.log(props.x), 0);
  return <div />;
}
function ThenCallback(props: { x: number }) {
  void Promise.resolve(1).then(() => console.log(props.x));
  return <div />;
}
// A stored literal.
function Stored(props: { x: number }) {
  const bag = { run: () => console.log(props.x) };
  return <div>{typeof bag}</div>;
}
// A modelled layer inside an unmodelled one: the outer callback is a timer's.
function IifeInTimer(props: { x: number }) {
  setTimeout(() => {
    (() => console.log(props.x))();
  }, 0);
  return <div />;
}
function ForEachInTimer(props: { k: number }) {
  setTimeout(() => {
    [1].forEach((x) => console.log(x + props.k));
  }, 0);
  return <div />;
}
function StoreOnAttribute() {
  const [s] = createStore({ a: 1 });
  return <Button onPick={() => console.log(s.a)} />;
}
function StoreUnknown() {
  const [s] = createStore({ a: 1 });
  opaqueCb(() => console.log(s.a));
  return <div />;
}
// A helper inside an unmodelled callback stays silent.
function HelperInUnknown(props: { x: number }) {
  opaqueCb(() => {
    const g = () => props.x;
    console.log(g());
  });
  return <div />;
}
function HelperInTimer(props: { x: number }) {
  setTimeout(() => {
    const g = () => props.x;
    console.log(g());
  }, 0);
  return <div />;
}
function now(f: () => void) {
  f();
}
declare function sink(x: unknown): void;
// A helper handed to unknown code, or run only from an event: option B's silence.
function HelperEscapes(props: { x: number }) {
  now(() => {
    const g = () => props.x;
    sink(g);
  });
  return <div />;
}
function HelperEventOnly(props: { x: number }) {
  return (
    <div
      onClick={() => {
        const g = () => props.x;
        sink(g());
      }}
    />
  );
}
// An anonymous renderer is not a component: its parameter is no props object.
function Renderer() {
  const columns = [{ render: (row: { name: string }) => <div>{row.name}</div> }];
  return <div>{columns.length}</div>;
}

export function Root() {
  const [n] = createSignal(1);
  return (
    <div>
      <OnAttribute x={n()} />
      <UnknownCallee x={n()} />
      <Timer x={n()} />
      <ThenCallback x={n()} />
      <Stored x={n()} />
      <IifeInTimer x={n()} />
      <ForEachInTimer k={n()} />
      <StoreOnAttribute />
      <StoreUnknown />
      <HelperInUnknown x={n()} />
      <HelperInTimer x={n()} />
      <HelperEscapes x={n()} />
      <HelperEventOnly x={n()} />
      <Renderer />
    </div>
  );
}
