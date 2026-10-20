import { createEffect, createMemo, createSignal } from "solid-js";
type Runner = { run: () => void };
function wrap(value: unknown) {
  sink({ value });
}
export function SignalPrimitive() {
  const [count, setCount] = createSignal(0);
  setCount(1);
  setCount((previous) => previous + 1);
  wrap(count());
  return <main />;
}
export function SignalObjectWrite() {
  const [value, setValue] = createSignal<number | Runner>(0);
  setValue(opaque);
  wrap(value());
  return <main />;
}
export function SignalSetterEscapes() {
  const [count, setCount] = createSignal(0);
  register(setCount);
  wrap(count());
  return <main />;
}
export function SignalUndefinedInitial() {
  const [count] = createSignal<number | undefined>(undefined);
  wrap(count());
  return <main />;
}
export function MemoPrimitive() {
  const [count] = createSignal(0);
  const label = createMemo(() => `n=${count()}`);
  wrap(label());
  return <main />;
}
export function MemoObject() {
  const picked = createMemo(() => opaque);
  wrap(picked());
  return <main />;
}
export function EffectApplyPrimitive() {
  const [count] = createSignal(0);
  createEffect(() => count() * 2, (doubled) => {
    wrap(doubled);
  });
  return <main />;
}
export function EffectApplyObject() {
  createEffect(() => opaque, (value) => {
    wrap(value);
  });
  return <main />;
}
export function HelperParameterPrimitive() {
  const show = (n: number) => wrap(n);
  show(1);
  show(2 + 3);
  return <main />;
}
export function HelperParameterAsserted() {
  const show = (n: number) => wrap(n);
  show(1);
  show(opaque as unknown as number);
  return <main />;
}
function Label(props: { text: string }) {
  wrap(props.text);
  return <span />;
}
function LabelAsserted(props: { text: string }) {
  wrap(props.text);
  return <span />;
}
function LabelMissing(props: { text?: string }) {
  wrap(props.text);
  return <span />;
}
export function Props() {
  return (
    <main>
      <Label text="a" />
      <Label text={`b${1}`} />
      <LabelAsserted text={opaque as unknown as string} />
      <LabelMissing />
    </main>
  );
}
function describe(n: number) {
  return `#${n}`;
}
function maybe(n: number) {
  if (n > 0) return "x";
}
export function FunctionResult() {
  wrap(describe(1));
  return <main />;
}
export function FunctionFallsThrough() {
  wrap(maybe(1));
  return <main />;
}
function readId(record: { id: string; run: () => void }) {
  sink(record.id);
}
export function LiteralMember() {
  readId({ id: "a", run: () => undefined });
  return <main />;
}
export function LiteralMemberAsserted() {
  readId({ id: opaque as unknown as string, run: () => undefined });
  return <main />;
}
declare const opaque: Runner;
declare function sink(value: unknown): void;
declare function register(value: unknown): void;
