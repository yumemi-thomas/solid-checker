import { createSignal, untrack } from "solid-js";

// ADR 1184: a prop read in an anonymous literal called where it is written (an IIFE), inside a named local function the
// component calls. The read runs untracked in the render (Chrome: STRICT_READ_UNTRACKED), but no summary carries a prop read of
// the IIFE to the call: it was certified clean. Now SC1001 uncertifiable, never silent.
function Arrow(props: { value: number }) {
  const go = () => {
    (() => {
      props.value;
    })();
  };
  go();
  return <i />;
}

// The census shape: a ref factory the render calls, the prop read through a type assertion.
function RefFactory(props: { value: number }) {
  const dir = () => {
    (() => {
      (props as typeof props).value;
    })();
    return () => {};
  };
  return <p ref={dir()}>x</p>;
}

// A function expression called where it is written.
function Expression(props: { value: number }) {
  const go = () => {
    (function () {
      props.value;
    })();
  };
  go();
  return <i />;
}

// Kept: the read in the named function itself is reported at `go()` (violation).
function Direct(props: { value: number }) {
  const go = () => {
    props.value;
  };
  go();
  return <i />;
}

// Kept: an IIFE in the body is the body's (violation at the read).
function BodyIife(props: { value: number }) {
  (() => {
    props.value;
  })();
  return <i />;
}

// Kept: an accessor read in the same IIFE is carried to `go()` (violation there).
function AccessorIife() {
  const [count] = createSignal(0);
  const go = () => {
    (() => {
      count();
    })();
  };
  go();
  return <i />;
}

// Kept: the read under untrack does not warn (clean).
function Untracked(props: { value: number }) {
  const go = () => {
    untrack(() => props.value);
  };
  go();
  return <i />;
}

// Kept: an IIFE reading a plain object, no prop (clean).
function PlainObject() {
  const box = { value: 1 };
  const go = () => {
    (() => {
      box.value;
    })();
  };
  go();
  return <i />;
}

export function Page() {
  const [value] = createSignal(1);
  return (
    <div>
      <Arrow value={value()} />
      <RefFactory value={value()} />
      <Expression value={value()} />
      <Direct value={value()} />
      <BodyIife value={value()} />
      <AccessorIife />
      <Untracked value={value()} />
      <PlainObject />
    </div>
  );
}
