import { createEffect, createMemo, createSignal, mapArray, untrack } from "solid-js";
import { consume } from "unknown-package";

export function tracked(read: () => number) {
  return createMemo(read);
}
export function trackedTwice(read: () => number) {
  const first = createMemo(() => read());
  const second = createMemo(() => read());
  return [first, second] as const;
}
export function effectCompute(read: () => number) {
  createEffect(read, (value) => { void value; });
}
export function computedSignal(read: () => number) {
  return createSignal(read);
}
export function explicitUntrack(read: () => number) {
  return untrack(read);
}
export function mixedSafe(read: () => number) {
  untrack(read);
  return createMemo(read);
}
export function queued(read: () => number) {
  queueMicrotask(read);
}
export const inline = (read: () => number) => read();
export function trackedAndInline(read: () => number) {
  const result = createMemo(read);
  read();
  return result;
}
let saved: (() => number) | undefined;
export function stored(read: () => number) {
  saved = read;
}
export function replaySaved() {
  return saved?.();
}
export function returned(read: () => number) {
  return read;
}
export function returnedUntrack(read: () => number) {
  return () => untrack(read);
}
export function unknown(read: () => number) {
  consume(read);
}
export function synchronousEvent(read: () => number) {
  const element = document.createElement("button");
  element.addEventListener("probe", read);
  element.dispatchEvent(new Event("probe"));
}
export function thenable(read: () => number, promise: PromiseLike<void>) {
  promise.then(read);
}
export function recursive(read: () => number, depth: number) {
  if (depth <= 0) return;
  recursive(read, depth - 1);
}
export async function asyncConsumer(read: () => number) {
  await Promise.resolve();
  return untrack(read);
}
export function defaultConsumer(read: () => number = () => 0) {
  return createMemo(read);
}
export function TrackedProp(props: { read: () => number }) {
  const value = createMemo(() => props.read());
  return <div>{value()}</div>;
}
export function TrackedJsx(props: { read: () => number }) {
  return <div>{props.read()}</div>;
}
export function ForwardedProp(props: { read: () => number }) {
  return <TrackedProp read={props.read} />;
}
export function EagerProp(props: { read: () => number }) {
  const value = props.read();
  return <div>{value}</div>;
}
export function StoredProp(props: { read: () => number }) {
  saved = props.read;
  return <div />;
}
export function ReturnedProps(props: { read: () => number }) {
  publishProps(props);
  return <div>{untrack(props.read)}</div>;
}
declare function publishProps(props: { read: () => number }): void;
export function objectSlot(options: { key: () => number; load: () => number }) {
  const result = createMemo(() => options.key());
  saved = options.load;
  return result;
}
// Review counterexamples: a sibling field that runs with the object as `this`.
export function siblingInvoke(o: { key: () => number; invoke: () => number }) {
  const result = createMemo(o.key);
  o.invoke();
  return result;
}
export function siblingValueOf(o: { key: () => number }) {
  const result = createMemo(o.key);
  const alias = o.valueOf() as typeof o;
  alias.key();
  return result;
}
export function siblingTag(o: { key: () => number; tag: (strings: TemplateStringsArray) => number }) {
  const result = createMemo(o.key);
  o.tag`probe`;
  return result;
}
// A strict-read label opens its own warning window.
export function labelledUntrack(read: () => number) {
  return untrack(read, "labelled consumer");
}
function stop(): number {
  throw new Error("skip this invocation");
}
// The default throws before the body runs, so `read` is never called.
export const gatedInline = (read: () => number, _gate = stop()) => read();
export const forwardGated = (read: () => number) => gatedInline(read);
// An invoked sibling prop is harmless when the site passes an arrow, which
// has no `this` of its own.
export function SiblingArrowProp(props: { read: () => number; check?: (n: number) => boolean }) {
  const value = createMemo(() => props.read());
  const checked = props.check?.(1);
  return <div>{value()}{String(checked)}</div>;
}
// Round 2 review counterexamples.
type Overwritable = { key: () => number; invoke: () => number };
export function siblingOverwritten(o: Overwritable) {
  const result = createMemo(o.key);
  o.invoke = function (this: Overwritable) { return this.key(); };
  o.invoke();
  return result;
}
type WithProto = { key: () => number; invoke?: () => number; __proto__?: object };
export function prototypeWritten(o: WithProto) {
  const result = createMemo(o.key);
  o.__proto__ = { invoke(this: WithProto) { return this.key(); } };
  o.invoke?.();
  return result;
}
type ProtoProps = { read: () => number; invoke?: () => number; __proto__?: object };
export function ProtoConsumer(props: ProtoProps) {
  const value = createMemo(props.read);
  props.invoke?.();
  return <div>{value()}</div>;
}
export function labelledInsideMemo(read: () => number) {
  return createMemo(() => untrack(read, "labelled inside memo"));
}
function EagerChild(props: { read: () => number }) {
  props.read();
  return <div />;
}
function forwardToChild(read: () => number) {
  return <EagerChild read={read} />;
}
export function componentInsideMemo(read: () => number) {
  return createMemo(() => forwardToChild(read));
}
const twoArgInline = (read: () => number, _gate: number) => read();
export const forwardWithGate = (read: () => number) => twoArgInline(read, gate);
const gate = 0;
// Round 3 review counterexamples.
export function siblingRead(options: { read: () => number; probe?: number }) {
  const result = createMemo(options.read);
  void options.probe;
  return result;
}
export function unnamedRows(read: () => number) {
  return mapArray(() => [0], read);
}
// `name` wraps the map function in a labelled strict-read window.
export function namedRows(read: () => number) {
  return mapArray(() => [0], read, { name: "review row" });
}
// `catch` calls its receiver's `then`, which a subclass may override.
export function caught(read: () => number) {
  void Promise.resolve(0).catch(read);
}
// Round 4: a `function` callback receives the props object as `this`.
export let replay: () => number = () => 0;
export function ReceiverProp(props: { read: () => number; probe?: number }) {
  const value = createMemo(() => props.read());
  void props.probe;
  return <div>{value()}</div>;
}
