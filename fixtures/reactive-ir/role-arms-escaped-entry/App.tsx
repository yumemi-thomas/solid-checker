import { createEffect, createMemo, createSignal } from "solid-js";

// ADR 1119b: the four role arms the proven-silences census flagged with an open entry census (`compiler-source`,
// `owner-context-current`, `window_after_awaits.unreported`, `write_role_walk.unreported`), each reached by an entry no position
// records. Every site stays uncertifiable: the discovery's escape checks (a function stored, handed on or called through a value)
// leave it unplaced before a role arm answers. Chrome warns at each; none may become clean.

// compiler-source: a function created in a JSX expression or a handler, called later from an effect's apply.
export function JsxGlobal() {
  const [s] = createSignal(1);
  createEffect(() => 1, () => {
    (globalThis as any).fJsxGlobal?.();
  });
  return <div>{(() => { const f = () => s(); (globalThis as any).fJsxGlobal = f; return 1; })()}</div>;
}

export function JsxLet() {
  const [s] = createSignal(1);
  let g: (() => number) | undefined;
  createEffect(() => 1, () => {
    g?.();
  });
  return <div>{(() => { g = () => s(); return 1; })()}</div>;
}

export function JsxRef() {
  const [s] = createSignal(1);
  return <div ref={(() => { const f = (el: HTMLDivElement) => { void el; s(); }; return f; })()}>x</div>;
}

export function HandlerClosureGlobal() {
  const [s] = createSignal(1);
  const [n, setN] = createSignal(0);
  createEffect(() => n(), () => {
    (globalThis as any).kHandlerGlobal?.();
  });
  return <button onClick={() => { const hf = () => { s(); }; (globalThis as any).kHandlerGlobal = hf; setN(1); }}>x</button>;
}

export function HandlerClosureLet() {
  const [s] = createSignal(1);
  const [n, setN] = createSignal(0);
  let k: (() => void) | undefined;
  createEffect(() => n(), () => {
    k?.();
  });
  return <button onClick={() => { const hf = () => { s(); }; k = hf; setN(1); }}>x</button>;
}

export function HandlerClosureArray() {
  const [s] = createSignal(1);
  const [n, setN] = createSignal(0);
  let k: (() => void)[] = [];
  createEffect(() => n(), () => {
    k.forEach((g) => g());
  });
  return <button onClick={() => { const hf = () => { s(); }; k = [hf]; setN(1); }}>x</button>;
}

// owner-context-current: a creation in a function the body calls, also run on an effect's apply.
export function OwnerForEach() {
  const f = () => {
    createMemo(() => 1);
  };
  f();
  createEffect(() => 1, () => {
    [f].forEach((g) => g());
  });
  return <div />;
}

export function OwnerLet() {
  const f = () => {
    createMemo(() => 1);
  };
  f();
  let g: (() => void) | undefined = f;
  createEffect(() => 1, () => {
    g?.();
  });
  return <div />;
}

export function OwnerAlias() {
  const f = () => {
    createMemo(() => 1);
  };
  f();
  const g = f;
  createEffect(() => 1, () => {
    g();
  });
  return <div />;
}

export function OwnerObject() {
  const f = () => {
    createMemo(() => 1);
  };
  f();
  const o = { f };
  createEffect(() => 1, () => {
    o.f();
  });
  return <div />;
}

// window_after_awaits / write_role_walk.unreported: a closure created after an await, called later in a window or an owned scope.
export function AwaitReadLet() {
  const [s] = createSignal(1);
  const [n, setN] = createSignal(0);
  let g: (() => number) | undefined;
  void (async () => {
    await 0;
    g = () => s();
  })();
  createEffect(() => n(), () => {
    g?.();
  });
  return <button onClick={() => setN(1)}>x</button>;
}

export function AwaitReadGlobal() {
  const [s] = createSignal(1);
  const [n, setN] = createSignal(0);
  void (async () => {
    await 0;
    (globalThis as any).gAwaitRead = () => s();
  })();
  createEffect(() => n(), () => {
    (globalThis as any).gAwaitRead?.();
  });
  return <button onClick={() => setN(1)}>x</button>;
}

export function AwaitWriteLet() {
  const [s, setS] = createSignal(1);
  const [n, setN] = createSignal(0);
  let g: (() => void) | undefined;
  void (async () => {
    await 0;
    g = () => setS(2);
  })();
  const m = createMemo(() => {
    n();
    g?.();
    return 1;
  });
  return <button onClick={() => setN(1)}>{m()}{s()}</button>;
}

export function AwaitWriteGlobal() {
  const [s, setS] = createSignal(1);
  const [n, setN] = createSignal(0);
  void (async () => {
    await 0;
    (globalThis as any).hAwaitWrite = () => setS(2);
  })();
  const m = createMemo(() => {
    n();
    (globalThis as any).hAwaitWrite?.();
    return 1;
  });
  return <button onClick={() => setN(1)}>{m()}{s()}</button>;
}
