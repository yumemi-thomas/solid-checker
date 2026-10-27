import { createMemo, createSignal, onCleanup, onSettled } from "solid-js";

declare const flag: boolean;
const count = (): number => 0;

function runBody(cb: () => void) {
  cb();
  return 1;
}

// ---- A call in the parameter list of a closure a helper builds runs only when
// that closure is called without the argument (ADR 0454).

function buildNested(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g;
}
function buildForward(cb: () => void) {
  const g = (x: unknown = runBody(cb)) => x;
  return g;
}
function buildConditional(cb: () => void) {
  if (flag) {
    const g = (x: unknown = cb()) => x;
    return g;
  }
  return undefined;
}
function buildDirect(cb: () => void) {
  return (x: unknown = cb()) => x;
}
function buildDestructured(cb: () => void) {
  return ({ a = cb() }: { a?: unknown }) => a;
}
function buildDeclaration(cb: () => void) {
  function g(x: unknown = cb()) {
    return x;
  }
  return g;
}
function buildIife(cb: () => void) {
  const g = (x: unknown = (() => cb())()) => x;
  return g;
}
function buildAndCall(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g();
}
function buildAndCallSupplied(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g(5);
}
// The helper's own parameter list: it runs only when a caller omits `x`.
function runOwnDefault(cb: () => void, x: unknown = cb()) {
  return x;
}

// ---- Writes. Never called: nothing runs the default, rc.17 throws nothing.

export function NestedNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildNested(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function ForwardNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildForward(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function ConditionalNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildConditional(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function DirectNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildDirect(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function DestructuredNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildDestructured(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function DeclarationNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildDeclaration(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function IifeNeverCalled() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildIife(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
// The closure is built here and called later, from an event: legal.
export function NestedCalledInHandler() {
  const [v, setV] = createSignal(0);
  const closure = buildNested(() => setV(1));
  const m = createMemo(() => 1);
  return <button onClick={() => closure()}>{m()}{v()}</button>;
}

// ---- Called. The argument is omitted, so the default runs in the memo.

export function NestedCalledOmitted() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildAndCall(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function NestedCalledSupplied() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildAndCallSupplied(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function OwnDefaultOmitted() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runOwnDefault(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function BodyCall() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runBody(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}

// ---- A leaf scope: a call in a nested closure's default is not a call of the
// leaf callback.

export function LeafNestedDefault() {
  onSettled(() => {
    const g = (x: unknown = onCleanup(() => {})) => x;
    void g;
  });
  return <div />;
}
export function LeafDirect() {
  onSettled(() => {
    onCleanup(() => {});
  });
  return <div />;
}

// ---- Reads. A helper that builds a closure whose default calls the accessor
// passed in: the component body reads nothing unless the closure is called.

function readNested(read: () => number) {
  const g = (x: unknown = read()) => x;
  return g;
}
function readAndCall(read: () => number) {
  const g = (x: unknown = read()) => x;
  return g();
}
function readAndCallSupplied(read: () => number) {
  const g = (x: unknown = read()) => x;
  return g(5);
}
function readBody(read: () => number) {
  return read();
}

export function ReadNestedNeverCalled() {
  const [n] = createSignal(0);
  readNested(n);
  return <div />;
}
export function ReadNestedCalledOmitted() {
  const [n] = createSignal(0);
  readAndCall(n);
  return <div />;
}
export function ReadNestedCalledSupplied() {
  const [n] = createSignal(0);
  readAndCallSupplied(n);
  return <div />;
}
export function ReadBodyCall() {
  const [n] = createSignal(0);
  readBody(n);
  return <div />;
}

// Controls: the closure's body, not its default, calls the accessor.
function readBodyClosureNested(read: () => number) {
  const g = () => read();
  return g;
}
function readBodyClosureCalled(read: () => number) {
  const g = () => read();
  return g();
}
export function ReadBodyClosureNeverCalled() {
  const [n] = createSignal(0);
  readBodyClosureNested(n);
  return <div />;
}
export function ReadBodyClosureCalled() {
  const [n] = createSignal(0);
  readBodyClosureCalled(n);
  return <div />;
}

function readOwnDefault(read: () => number, x: unknown = read()) {
  return x;
}
export function ReadOwnDefaultOmitted() {
  const [n] = createSignal(0);
  readOwnDefault(n);
  return <div />;
}
export function ReadOwnDefaultSupplied() {
  const [n] = createSignal(0);
  readOwnDefault(n, 5);
  return <div />;
}

// ---- An anonymous callback's own list: whether its default runs depends on
// the arguments the invoker supplies, which nothing here proves.

export function ForEachDefault() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    [1].forEach((item, index = setV(1)) => void item); // `forEach` supplies the index
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function IifeDefault() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    ((x = setV(1)) => x)(); // omitted, so it runs: known miss, uncertifiable
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function PatternDefault() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    [{ a: 1 }].map(({ a = setV(1) }) => a); // `a` is supplied
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
