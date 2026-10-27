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
