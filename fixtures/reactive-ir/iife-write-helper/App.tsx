import { action, createSignal } from "solid-js";

// ADR 1212: a write written directly in an IIFE inside a named local function runs at the IIFE's call, in that function: each call of the
// function runs it there, as ADR 1187 charges a prop read. The component body is an owned scope: Solid throws REACTIVE_WRITE_IN_OWNED_SCOPE
// (and ACTION_CALLED_IN_OWNED_SCOPE) at the write.

const act = action(function* () {
  yield 1;
});

// Violation: the helper is called in the body.
export function BodyCall() {
  const [w, setW] = createSignal(0);
  const go = () => {
    (() => {
      setW(1);
    })();
  };
  go();
  return <p>{w()}</p>;
}

// Violation: two IIFE layers, the helper called by a ref factory the body evaluates.
export function RefFactory() {
  const [w, setW] = createSignal(0);
  const dir = () => {
    (() => {
      (() => {
        setW(2);
      })();
    })();
    return () => {};
  };
  return <p ref={dir()}>{w()}</p>;
}

// Violation: an action call in the IIFE.
export function ActionCall() {
  const go = () => {
    (() => {
      act();
    })();
  };
  go();
  return <p>a</p>;
}

// Clean: the helper runs only from an event handler, where a write is legal.
export function Handler() {
  const [w, setW] = createSignal(0);
  const go = () => {
    (() => {
      setW(3);
    })();
  };
  return <button onClick={() => go()}>{w()}</button>;
}

// Kept uncertifiable (the member variant): the literal is run through `.call`, not called where it is written.
export function ThroughCall() {
  const [w, setW] = createSignal(0);
  const go = () => {
    (function () {
      setW(4);
    }).call(undefined);
  };
  go();
  return <p>{w()}</p>;
}

// Kept uncertifiable: a named function expression may call itself by name from anywhere it runs.
export function SelfNamed() {
  const [w, setW] = createSignal(0);
  const go = () => {
    (function again(n: number) {
      if (n > 0) setW(n);
    })(1);
  };
  go();
  return <p>{w()}</p>;
}


// Kept uncertifiable (the shadowed variant): the binding is reassigned before its call, so the function holding the IIFE never runs.
export function Reassigned() {
  const [w, setW] = createSignal(0);
  let go = () => {
    (() => {
      setW(5);
    })();
  };
  go = () => {};
  go();
  return <p>{w()}</p>;
}
