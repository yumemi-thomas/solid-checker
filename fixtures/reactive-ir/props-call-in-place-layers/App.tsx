import { createSignal, omit } from "solid-js";

// ADR 1222: a call of a prop in layers that each run once, in place, in the
// component body (a local closure's one direct call, an IIFE) runs on the
// body's stack, so it is placed as if written there.

type P = { setW: (value: number) => void; extra?: number };
declare function later(f: () => void): void;

// Misuse: a closure called once in the body writes the signal.
function ClosureOnce(props: P) {
  const f = () => {
    props.setW(7);
  };
  f();
  return <p />;
}
// Misuse: an IIFE in the body.
function Iife(props: P) {
  (() => {
    props.setW(7);
  })();
  return <p />;
}
// Misuse: two layers, each called once.
function Nested(props: P) {
  const outer = () => {
    const inner = () => {
      props.setW(7);
    };
    inner();
  };
  outer();
  return <p />;
}
// Misuse: the member of an `omit` view (ADR 1100) in a closure called once.
function OmitView(props: P) {
  const rest = omit(props, "extra");
  const f = () => {
    rest.setW(7);
  };
  f();
  return <p />;
}
// Uncertifiable: the closure runs twice, one run among others.
function Twice(props: P) {
  const f = () => {
    props.setW(7);
  };
  f();
  f();
  return <p />;
}
// Uncertifiable: the closure is handed to a function nothing models.
function Handed(props: P) {
  const f = () => {
    props.setW(7);
  };
  later(f);
  return <p />;
}
// Legal: the closure runs from an event handler.
function Handler(props: P) {
  const f = () => {
    props.setW(7);
  };
  return <p onClick={() => f()} />;
}
// Silent: a local `props` shadows the parameter inside the closure.
function Shadowed(props: P) {
  const f = () => {
    const props = { setW: (value: number) => value };
    props.setW(7);
  };
  f();
  return <p />;
}

export function App() {
  const [, setW] = createSignal(0);
  return (
    <p>
      <ClosureOnce setW={setW} />
      <Iife setW={setW} />
      <Nested setW={setW} />
      <OmitView setW={setW} />
      <Twice setW={setW} />
      <Handed setW={setW} />
      <Handler setW={setW} />
      <Shadowed setW={setW} />
    </p>
  );
}
