import * as Solid from "solid-js";
import { createSignal, createStore, flush } from "solid-js";

// ADR 1161: `flush(fn)` runs `fn` on its caller's stack and keeps the caller's strict-read window. When the call is in a
// closure that only in-place layers run (called once by name and referenced nowhere else, an IIFE), the callback runs where
// the outermost layer's call runs: the body, a JSX expression, a handler.

// Violations: the closure is called once in the body, where the window is open (STRICT_READ_UNTRACKED).
function ClosureInBody() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
    return 1;
  };
  run();
  return <i />;
}

function IifeInClosure() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      (() => {
        count();
      })();
    });
    return 1;
  };
  run();
  return <i />;
}

function StoreMemberInBody() {
  const [store] = createStore({ a: { v: 0 } });
  const run = () => {
    flush(() => {
      void store.a;
    });
    return 1;
  };
  run();
  return <i />;
}

function NamespaceFlushInBody() {
  const [count] = createSignal(3);
  const run = () => {
    Solid.flush(() => {
      count();
    });
    return 1;
  };
  run();
  return <i />;
}

// Clean: the closure is called once in a JSX expression (tracked) and once in a handler (no window).
function ClosureInJsx() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
    return 1;
  };
  return <p>{(() => { run(); return 1; })()}</p>;
}

function ClosureInHandler() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
  };
  return <button onClick={() => run()}>go</button>;
}

// Uncertifiable, as before: the closure is also handed to a timer, called twice, or handed to `forEach`.
function AlsoTimer() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
  };
  run();
  setTimeout(run, 0);
  return <i />;
}

function CalledTwice() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
  };
  run();
  return <button onClick={() => run()}>go</button>;
}

function HandedToForEach() {
  const [count] = createSignal(3);
  const run = () => {
    flush(() => {
      count();
    });
  };
  return <p>{(() => { [1].forEach(run); return 1; })()}</p>;
}

export function Page() {
  return (
    <div>
      <ClosureInBody />
      <IifeInClosure />
      <StoreMemberInBody />
      <NamespaceFlushInBody />
      <ClosureInJsx />
      <ClosureInHandler />
      <AlsoTimer />
      <CalledTwice />
      <HandedToForEach />
    </div>
  );
}
