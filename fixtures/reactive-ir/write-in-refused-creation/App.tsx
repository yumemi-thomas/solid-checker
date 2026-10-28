import {
  action,
  createEffect,
  createRoot,
  createSignal,
  createTrackedEffect,
  onSettled,
} from "solid-js";

const [w, setW] = createSignal(0);

// Uncertifiable: rc.17 throws PRIMITIVE_IN_EFFECT_CALLBACK at the root call.
export function ApplyRootInline() {
  createEffect(() => 1, () => { createRoot(() => { setW(1); }); });
  return <div>{w()}</div>;
}

export function ApplyRootNamedClosure() {
  createEffect(() => 1, () => { createRoot(() => { const g = () => setW(2); g(); }); });
  return <div>{w()}</div>;
}

export function ApplyRootByName() {
  function f1() { setW(3); }
  createEffect(() => 1, () => { createRoot(f1); });
  return <div>{w()}</div>;
}

export function ApplyRootAction() {
  const a1 = action(function* () { setW(4); });
  createEffect(() => 1, () => { createRoot(() => { a1(); }); });
  return <div>{w()}</div>;
}

// Uncertifiable: rendering creates a root first, which rc.17 refuses.
function SettledChild() { setW(5); return <span />; }
export function RenderInSettled() {
  onSettled(() => { <SettledChild />; });
  return <div>{w()}</div>;
}

function ApplyChild() { setW(6); return <span />; }
export function RenderInApply() {
  createEffect(() => 1, () => { <ApplyChild />; });
  return <div>{w()}</div>;
}

function TrackedChild() { setW(7); return <span />; }
export function RenderInTrackedEffect() {
  createTrackedEffect(() => { <TrackedChild />; });
  return <div>{w()}</div>;
}

export function NestedChildInSettled() {
  function Inner() { setW(8); return <span />; }
  onSettled(() => { <Inner />; });
  return <div>{w()}</div>;
}

// Violations: a run that happens.
function BodyChild() { setW(9); return <span />; }
export function RenderInBody() {
  return <div>{w()}<BodyChild /></div>;
}

function TwiceChild() { setW(10); return <span />; }
export function RenderInSettledAndBody() {
  onSettled(() => { <TwiceChild />; });
  return <div>{w()}<TwiceChild /></div>;
}

export function TimerRoot() {
  createEffect(() => 1, () => { setTimeout(() => createRoot(() => { setW(11); }), 0); });
  return <div>{w()}</div>;
}

export function HandlerRoot() {
  return <button onClick={() => createRoot(() => { setW(12); })}>{w()}</button>;
}
