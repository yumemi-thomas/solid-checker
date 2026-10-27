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
