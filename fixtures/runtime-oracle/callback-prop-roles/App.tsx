import { createEffect, createSignal } from "solid-js";

// ADR 1190 (slice 2): a read or write in a callback prop's literal takes the role of the child's calls that run it.

// A read in the literal, called in the child's effect apply: STRICT_READ_UNTRACKED. SC1001 violation (main: uncertifiable).
function ApplyKid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p>x</p>;
}
export function ApplyRead() {
  const [r] = createSignal(1);
  return (
    <ApplyKid
      run={() => {
        r();
      }}
    />
  );
}

// A named function's read, called in the child's body: SC1001 violation (main: uncertifiable).
function BodyKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
export function NamedBodyRead() {
  const [r] = createSignal(1);
  function run1() {
    r();
  }
  return <BodyKid run={run1} />;
}

// A destructured prop's write, called in the child's body: REACTIVE_WRITE_IN_OWNED_SCOPE. SC2001 violation (main: uncertifiable).
function DestructuredBodyKid({ run }: { run: () => void }) {
  run();
  return <p>x</p>;
}
export function DestructuredBodyWrite() {
  const [, setW] = createSignal(0);
  return (
    <DestructuredBodyKid
      run={() => {
        setW(1);
      }}
    />
  );
}

// The same write called from the child's handler: clean (main: uncertifiable).
function DestructuredHandlerKid({ run }: { run: () => void }) {
  return <button onClick={() => run()}>go</button>;
}
export function DestructuredHandlerWrite() {
  const [, setW] = createSignal(0);
  return (
    <DestructuredHandlerKid
      run={() => {
        setW(1);
      }}
    />
  );
}

// A read in an IIFE in the literal, called in the child's tracked JSX: no warning, no row (the first cut's false positive).
function JsxKid({ run }: { run: () => void }) {
  return <p>{(run(), "x")}</p>;
}
export function JsxIifeRead() {
  const [r] = createSignal(1);
  return (
    <JsxKid
      run={() => {
        (() => {
          r();
        })();
      }}
    />
  );
}

// The child calls the prop from a local handler it calls in its body and also hands to `onClick`: Chrome warns at the render's
// run. The call is in a named local function, whose own entries the role at the call does not check: SC1001 stays
// uncertifiable (`call_plainly_placed`).
function EagerKid(props: { run: () => void }) {
  const handler = () => props.run();
  handler();
  return <button onClick={handler}>go</button>;
}
export function EagerHandlerRead() {
  const [r] = createSignal(1);
  return (
    <EagerKid
      run={() => {
        r();
      }}
    />
  );
}
