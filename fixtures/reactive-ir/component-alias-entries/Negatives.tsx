import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Clean: the alias is only called, in the same tracked JSX (read attribution
// follows calls through an exact alias).
function Twice() {
  const value = count();
  return <b>{value}</b>;
}

const T2 = Twice;

// Clean: the alias is never used.
function Never() {
  const value = count();
  return <b>{value}</b>;
}

const N2 = Never;

// Clean: a type query is erased.
function Typed() {
  const value = count();
  return <b>{value}</b>;
}

type TypedResult = ReturnType<typeof Typed>;

// Clean: the alias is called in an event handler, which reads without an
// owner and does not track.
function Clicked() {
  const value = count();
  return <b>{value}</b>;
}

const K2 = Clicked;

// Clean: `Shadow` below is a local alias of `Quiet`, another symbol; the
// tag renders `Quiet`, not the outer `Shadow`.
function Shadow() {
  const value = count();
  return <b>{value}</b>;
}

function Quiet() {
  return <i />;
}

function ShadowHost() {
  const Shadow = Quiet;
  return <Shadow />;
}

export function Negatives(props: { result?: TypedResult }) {
  return (
    <p>
      {Twice()}
      {T2()}
      {Never()}
      {Typed()}
      {props.result}
      {Clicked()}
      <button onClick={() => K2()} />
      {Shadow()}
      <ShadowHost />
    </p>
  );
}
