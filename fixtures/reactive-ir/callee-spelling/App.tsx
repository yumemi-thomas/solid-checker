import { action, createMemo, createRoot, createSignal, onCleanup } from "solid-js";

// Violations: each spelling of a proven setter, action, helper or accessor
// call reports where the plain call does (ADR 0468).
export function WriteParens() {
  const [x, setX] = createSignal(0);
  createMemo(() => { (setX)(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteNestedParens() {
  const [x, setX] = createSignal(0);
  createMemo(() => { ((setX))(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteComma() {
  const [x, setX] = createSignal(0);
  createMemo(() => { (void 0, setX)(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteOptional() {
  const [x, setX] = createSignal(0);
  createMemo(() => { setX?.(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteNonNull() {
  const [x, setX] = createSignal(0);
  createMemo(() => { setX!(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteAssertion() {
  const [x, setX] = createSignal(0);
  createMemo(() => { (setX as (value: number) => number)(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteCommaInRender() {
  const [x, setX] = createSignal(0);
  return <p>{(() => (void 0, setX)(17))()}</p>;
}

export function ActionParens() {
  const a = action(function* () {});
  createMemo(() => { (a)(); return 0; });
  return <p />;
}

export function ActionComma() {
  const a = action(function* () {});
  createMemo(() => { (void 0, a)(); return 0; });
  return <p />;
}

export function ActionOptional() {
  const a = action(function* () {});
  createMemo(() => { a?.(); return 0; });
  return <p />;
}

export function ActionCommaInRoot() {
  const a = action(function* () {});
  createRoot(() => { (void 0, a)(); });
  return <p />;
}

export function CleanupCommaHelper() {
  setTimeout(() => {
    const f = () => onCleanup(() => {});
    (void 0, f)();
  }, 0);
  return <p />;
}

export function CleanupParensHelper() {
  setTimeout(() => {
    const f = () => onCleanup(() => {});
    (f)();
  }, 0);
  return <p />;
}

export function CleanupOptionalHelper() {
  setTimeout(() => {
    const f = () => onCleanup(() => {});
    f?.();
  }, 0);
  return <p />;
}

export function CleanupTwoHelpers() {
  setTimeout(() => {
    const f1 = () => onCleanup(() => {});
    const f2 = () => (f1)();
    (void 0, f2)();
  }, 0);
  return <p />;
}

export function CleanupCommaInThen() {
  const f = () => onCleanup(() => {});
  Promise.resolve().then(() => { (void 0, f)(); });
  return <p />;
}

export function CleanupCommaPrimitive() {
  setTimeout(() => { (void 0, onCleanup)(() => {}); }, 0);
  return <p />;
}

export function ReadComma() {
  const [x] = createSignal(0);
  (void 0, x)();
  return <p />;
}

// Violations: a helper that calls its parameter through each spelling.
export function CallbackComma() {
  const [x, setX] = createSignal(0);
  const run = (cb: () => void) => { (void 0, cb)(); };
  createMemo(() => { run(() => setX(1)); return 0; });
  return <p>{x()}</p>;
}

export function CallbackParens() {
  const [x, setX] = createSignal(0);
  const run = (cb: () => void) => { (cb)(); };
  createMemo(() => { run(() => setX(1)); return 0; });
  return <p>{x()}</p>;
}

export function CallbackOptional() {
  const [x, setX] = createSignal(0);
  const run = (cb: () => void) => { cb?.(); };
  createMemo(() => { run(() => setX(1)); return 0; });
  return <p>{x()}</p>;
}

// Silent: legal controls, a shadowing name, a target that is not called.
export function WriteCommaInTimer() {
  const [x, setX] = createSignal(0);
  setTimeout(() => { (void 0, setX)(1); }, 0);
  return <p>{x()}</p>;
}

export function ActionCommaInHandler() {
  const a = action(function* () {});
  return <button onClick={() => (void 0, a)()}>a</button>;
}

export function CleanupCommaInRoot() {
  const f = () => onCleanup(() => {});
  setTimeout(() => { createRoot(() => { (void 0, f)(); }); }, 0);
  return <p />;
}

export function ReadCommaTracked() {
  const [x] = createSignal(0);
  const m = createMemo(() => (void 0, x)());
  return <p>{m()}</p>;
}

export function WriteShadowed() {
  const [x, setX] = createSignal(0);
  {
    const setX = (value: number) => value;
    createMemo(() => { (void 0, setX)(1); return 0; });
  }
  return <p>{x()}</p>;
}

export function WriteNullishOptional() {
  const [x, setX] = createSignal(0);
  const s = Math.random() > 2 ? setX : undefined;
  createMemo(() => { s?.(1); return 0; });
  return <p>{x()}</p>;
}

// Uncertifiable: a comma, conditional or logical callee names no target; a
// written binding may hold another value.
export function WriteLogical() {
  const [x, setX] = createSignal(0);
  createMemo(() => { (true && setX)(1); return 0; });
  return <p>{x()}</p>;
}

export function WriteConditional() {
  const [x, setX] = createSignal(0);
  const [y, setY] = createSignal(0);
  const pick = Math.random() < 2;
  createMemo(() => { (pick ? setX : setY)(1); return 0; });
  return <p>{x()}{y()}</p>;
}

export function WriteEffectfulPrefix() {
  const [x, setX] = createSignal(0);
  const note = () => {};
  createMemo(() => { (note(), setX)(1); return 0; });
  return <p>{x()}</p>;
}

export function CleanupLogical() {
  setTimeout(() => {
    const f = () => onCleanup(() => {});
    (true && f)();
  }, 0);
  return <p />;
}

export function CleanupMemberTail() {
  setTimeout(() => {
    const o = { m() { onCleanup(() => {}); } };
    (0, o.m)();
  }, 0);
  return <p />;
}

export function CleanupReassigned() {
  setTimeout(() => {
    let k = () => { onCleanup(() => {}); };
    k = () => {};
    k();
  }, 0);
  return <p />;
}

export function CleanupNullifiedOptional() {
  setTimeout(() => {
    let k: (() => void) | undefined = () => { onCleanup(() => {}); };
    const clear = () => { k = undefined; };
    clear();
    k?.();
  }, 0);
  return <p />;
}

// Never a violation: `f && h` and `f = h` call `h`; a mention before its
// declarator throws (uncertifiable); a receiver-reading method run without its
// receiver throws; a bound function is a returned callable (silent: a known
// miss).
export function CleanupAndLeft() {
  setTimeout(() => {
    const f = (() => onCleanup(() => {})) as (() => void) | undefined;
    const h = () => {};
    ((f && h) as () => void)();
  }, 0);
  return <p />;
}

export function CleanupAssignedCallee() {
  setTimeout(() => {
    let f = () => { onCleanup(() => {}); };
    const h = () => {};
    (f = h)();
  }, 0);
  return <p />;
}

export function CleanupBeforeInit() {
  setTimeout(() => {
    const run = () => (void 0, early)();
    try { run(); } catch {}
    const early = () => onCleanup(() => {});
  }, 0);
  return <p />;
}

export function CleanupReceiverMethod() {
  setTimeout(() => {
    const o = { n: 1, m() { if (this.n) onCleanup(() => {}); } };
    try { (0, o.m)(); } catch {}
  }, 0);
  return <p />;
}

export function CleanupBind() {
  setTimeout(() => {
    const q = () => onCleanup(() => {});
    (void 0, q.bind(null))();
  }, 0);
  return <p />;
}
