import { createEffect, createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type Props = { w1: Accessor<number>; setW1: Setter<number> };

// ADR 1173: `Writer` is entered only through `Forward`'s `{...props}`, and `Forward` only by `Body`'s tag: `props.setW1` holds
// `Body`'s `createSignal` setter. Called in `Writer`'s body (under its owner): REACTIVE_WRITE_IN_OWNED_SCOPE, SC2001.
function Writer(props: Props) {
  props.setW1(7);
  return <p>writer</p>;
}
function Forward(props: Props) {
  return <Writer {...props} />;
}
export function Body() {
  const [w1, setW1] = createSignal(0);
  return <Forward w1={w1} setW1={setW1} />;
}

// The same setter called in an effect's apply: legal, silent (the prop read is static: no site passes a reactive value).
function ApplyWriter(props: Props) {
  createEffect(
    () => 0,
    () => {
      props.setW1(7);
    },
  );
  return <p>apply</p>;
}
function ApplyForward(props: Props) {
  return <ApplyWriter {...props} />;
}
export function Apply() {
  const [w1, setW1] = createSignal(0);
  return <ApplyForward w1={w1} setW1={setW1} />;
}

// The forward sets `setW1` itself after the spread: what `props.setW1` holds is not the forwarded value; no proof here.
function OverWriter(props: Props) {
  createEffect(
    () => 0,
    () => {
      props.setW1(7);
    },
  );
  return <p>over</p>;
}
function OverForward(props: Props) {
  const other: Setter<number> = ((v: number) => v) as Setter<number>;
  return <OverWriter {...props} setW1={other} />;
}
export function Override() {
  const [w1, setW1] = createSignal(0);
  return <OverForward w1={w1} setW1={setW1} />;
}

// The spread is no component's own props: no forward, no proof.
function RestWriter(props: Props) {
  createEffect(
    () => 0,
    () => {
      props.setW1(7);
    },
  );
  return <p>rest</p>;
}
export function Rest() {
  const [w1, setW1] = createSignal(0);
  const bag = { w1, setW1 };
  return <RestWriter {...bag} />;
}
