import { createSignal } from "solid-js";
import { Child } from "./children";

declare function load(): Promise<void>;

// The part of an async literal before its first await runs in the child's
// body, under its owner: both writes are SC2001.
export function BeforeAwait() {
  const [, setN] = createSignal(0);
  return (
    <Child
      run={async () => {
        setN(1);
        [1].forEach(() => setN(2));
        await load();
      }}
    />
  );
}

// After the await the literal resumes with no owner (ADR 0333): a write there
// is not placed in the body, directly, in a callback the standard library runs
// inline, or in an immediately invoked function.
export function AfterAwaitDirect() {
  const [, setN] = createSignal(0);
  return (
    <Child
      run={async () => {
        await load();
        setN(1);
      }}
    />
  );
}

export function AfterAwaitInline() {
  const [, setN] = createSignal(0);
  return (
    <Child
      run={async () => {
        await load();
        [1].forEach(() => setN(1));
      }}
    />
  );
}

export function AfterAwaitIife() {
  const [, setN] = createSignal(0);
  return (
    <Child
      run={async () => {
        await load();
        (() => setN(1))();
      }}
    />
  );
}

// The same through a project helper that calls its parameter: the helper's
// call is written past the await, in a callback the standard library runs
// inline.
function run(task: () => void) {
  task();
}

export function ViaHelperAfterAwait() {
  const [, setN] = createSignal(0);
  return (
    <Child
      run={async () => {
        await load();
        [1].forEach(() => run(() => setN(1)));
      }}
    />
  );
}
