import { createEffect, createMemo, createSignal, onSettled } from "solid-js";

// A closure bound to a name inside a tracked scope is constructed there, not
// run there. Its write throws only if a call runs it in the scope.

// ---- Uncertifiable: the closure is only returned or stored, so where it runs
// is the caller's business (here a click handler through the memo's value).

export function Returned() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const openFn = () => setOpen(true);
    return openFn;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function ReturnedDeclaration() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    function openFn() {
      setOpen(true);
    }
    return openFn;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function Stored() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const openFn = () => setOpen(true);
    return { openFn };
  });
  return <button onClick={() => api().openFn()}>{String(open())}</button>;
}

// Control: the anonymous form was already uncertifiable.
export function Inline() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => () => setOpen(true));
  return <button onClick={() => api()()}>{String(open())}</button>;
}

// ---- Violations: a call in the tracked scope's own pass runs the write there.

export function CalledInCompute() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const openFn = () => setOpen(true);
    openFn();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

export function CalledInComputeAndReturned() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const openFn = () => setOpen(true);
    openFn();
    return openFn;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function CalledInJsxExpression() {
  const [open, setOpen] = createSignal(false);
  return (
    <div>
      {(() => {
        const openFn = () => setOpen(true);
        openFn();
        return String(open());
      })()}
    </div>
  );
}

// ---- Nothing to report: the only call is in a leaf scope, which may write.

export function CalledInLeafScope() {
  const [open, setOpen] = createSignal(false);
  createEffect(
    () => {
      const openFn = () => setOpen(true);
      onSettled(() => openFn());
      return open();
    },
    () => {},
  );
  return <div />;
}

// ---- A write nested one level inside the named closure runs with it when
// every layer between runs in place (an IIFE, an inline standard-library
// callback): the closure's call in the scope's own pass is a violation.

export function CalledNestedInlineCallback() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = () => {
      [1].forEach(() => setOpen(true));
    };
    f();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

export function CalledNestedIife() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = () => {
      (() => setOpen(true))();
    };
    f();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

// The same nested writes in a closure only returned: nothing runs them here.
export function ReturnedNestedInlineCallback() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const f = () => {
      [1].forEach(() => setOpen(true));
    };
    return f;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

// ---- Other ways the scope reaches the closure.

export function CalledThroughAlias() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = () => setOpen(true);
    const g = f;
    g();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

export function CalledBeforeDeclaration() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    f();
    function f() {
      setOpen(true);
    }
    return open();
  });
  return <div>{String(doubled())}</div>;
}

export function CalledThroughOuterClosure() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const inner = () => setOpen(true);
    const outer = () => inner();
    outer();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

export function ReturnedOuterClosure() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const inner = () => setOpen(true);
    const outer = () => inner();
    return outer;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

// ---- A capitalized name does not decide: the calls do.

export function ReturnedCapitalized() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const Toggle = () => setOpen(true);
    return Toggle;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function CalledCapitalized() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const Toggle = () => setOpen(true);
    Toggle();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

// A capitalized closure that returns JSX is a possible component: that
// identity machinery decides it (uncertifiable), not the calls.
export function ReturnedJsxClosure() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const Row = () => {
      setOpen(true);
      return <div />;
    };
    return Row;
  });
  return <button onClick={() => api()}>{String(open())}</button>;
}

// ---- A default value runs when the function is called without the argument:
// the write in it is legal from a click (rc.17: nothing throws, the text
// becomes "open"), so a closure only returned or stored is unplaced, named or
// not, and so is a default inside a destructuring pattern.

export function ReturnedDefaultWrite() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const f = (x = setOpen(true)) => x;
    return f;
  });
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function ReturnedAnonymousDefaultWrite() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => (x = setOpen(true)) => x);
  return <button onClick={() => api()()}>{String(open())}</button>;
}

export function ReturnedDestructuringDefaultWrite() {
  const [open, setOpen] = createSignal(false);
  const api = createMemo(() => {
    const f = ({ a = setOpen(true) }: { a?: void }) => a;
    return f;
  });
  return <button onClick={() => api()({})}>{String(open())}</button>;
}

// Known miss: called in the compute with the argument left out this throws, but
// a function whose parameter list runs code has no call sites in the graph
// (ADR 0339), so nothing places the write.
export function CalledInComputeDefaultWrite() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = (x = setOpen(true)) => x;
    f();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

// The same limit for a write in the body of a closure with a non-literal default.
export function CalledInComputeNonLiteralDefault() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = (x = Math.random()) => setOpen(x > 0);
    f();
    return open();
  });
  return <div>{String(doubled())}</div>;
}

// A literal default leaves the call sites in the graph: still a violation.
export function CalledInComputeLiteralDefault() {
  const [open, setOpen] = createSignal(false);
  const doubled = createMemo(() => {
    const f = (x = 1) => setOpen(x > 0);
    f();
    return open();
  });
  return <div>{String(doubled())}</div>;
}
