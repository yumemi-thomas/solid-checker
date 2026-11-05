import { createMemo, createSignal, Loading } from "solid-js";

// `on` is the dependency's value: a function is never called (ADR 0624).
export function Accessors() {
  const [route] = createSignal(0);
  const page = createMemo(() => route() + 1);
  return (
    <>
      <Loading on={route} fallback="x"><p /></Loading>
      <Loading on={page} fallback="x"><p /></Loading>
      <Loading on={() => route()} fallback="x"><p /></Loading>
      <Loading on={function () { return route(); }} fallback="x"><p /></Loading>
    </>
  );
}

// A function by its type, wherever it came from.
export function Typed(props: { key: () => number }) {
  const [route, setRoute] = createSignal(0);
  return (
    <>
      <Loading on={setRoute} fallback="x"><p /></Loading>
      <Loading on={props.key} fallback="x"><p /></Loading>
      <Loading on={route} fallback="x"><p /></Loading>
    </>
  );
}

// A union that may be a function is unproven.
export function Mixed(props: { on: (() => number) | number }) {
  return <Loading on={props.on} fallback="x"><p /></Loading>;
}

// The value, or something this does not state.
export function Values(props: { on: unknown }) {
  const [route] = createSignal(0);
  const value = route();
  return (
    <>
      <Loading on={route()} fallback="x"><p /></Loading>
      <Loading on={[route(), value]} fallback="x"><p /></Loading>
      <Loading on={props.on} fallback="x"><p /></Loading>
      <Loading fallback="x"><p /></Loading>
    </>
  );
}

// A local name is not the primitive.
export function Shadowed() {
  const [route] = createSignal(0);
  const Loading = (props: { on?: unknown; children?: unknown }) => props.children;
  return <Loading on={route}><p /></Loading>;
}
