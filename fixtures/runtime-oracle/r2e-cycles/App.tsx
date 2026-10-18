import { createSignal } from "solid-js";

type P = { a?: () => void; b?: () => void; c?: () => void; d?: () => void; depth: number };

// a -> b -> c -> a rename cycle through three components; only C calls its
// `d` in its body, and `d` is filled from `c` on the second lap via D.
function A(props: P) {
  return <div onClick={props.a}>{props.depth > 0 && <B b={props.a} d={props.d} depth={props.depth - 1} />}</div>;
}
function B(props: P) {
  return <div onClick={props.b}>{props.depth > 0 && <C c={props.b} d={props.d} depth={props.depth - 1} />}</div>;
}
function C(props: P) {
  const f = props.d;
  if (f) f();
  return <div onClick={props.c}>{props.depth > 0 && <A a={props.c} d={props.c} depth={props.depth - 1} />}</div>;
}
export function ThreeCycle() {
  const [count] = createSignal(0);
  return <A a={() => console.log(count())} depth={6} />;
}

// Growing path: each level nests the value one object deeper.
type G = { h?: { x?: unknown }; depth: number };
function Grow(props: G) {
  const inner = props.h as { x?: () => void } | undefined;
  return <div onClick={inner?.x}>{props.depth > 0 && <Grow h={{ x: props.h }} depth={props.depth - 1} />}</div>;
}
export function Growing() {
  const [count] = createSignal(0);
  return <Grow h={{ x: () => console.log(count()) }} depth={3} />;
}
