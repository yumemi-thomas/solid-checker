import { createSignal } from "solid-js";

type Options = { onClose: () => void; label: string };

// `useThing` only reads members of its parameter, so the literal written as
// its argument is held by nothing else: its own `onClose` is what runs.
function useThing(options: Options) {
  options.onClose();
}

export function InertArrow() {
  useThing({ onClose: () => undefined, label: "a" });
  return <main />;
}

export function FunctionExpression() {
  useThing({ onClose: function () { return undefined; }, label: "a" });
  return <main />;
}

// The arrow's read stays claimed where it is written, as a callback's.
export function ReadingArrow() {
  const [open] = createSignal(false);
  useThing({ onClose: () => { open(); }, label: "a" });
  return <main />;
}

// Some other code writes a member named `onClose`; it cannot reach a literal
// only `useThing`'s parameter holds.
export function WrittenElsewhere() {
  useThing({ onClose: () => undefined, label: "a" });
  return <main />;
}
export function writeOnClose(target: Options) {
  target.onClose = () => undefined;
}

// A `function` expression that uses its receiver may hand the object on.
export function ReceiverFunction() {
  useThing({ onClose: function (this: unknown) { void this; }, label: "a" });
  return <main />;
}

// A spread after the property may replace it.
export function SpreadAfter() {
  useThing({ onClose: () => undefined, label: "a", ...extra });
  return <main />;
}

// A callee that hands its parameter on: the literal is not closed.
function handOn(options: Options) {
  options.onClose();
  sink(options);
}
export function HandedOn() {
  handOn({ onClose: () => undefined, label: "a" });
  return <main />;
}

// Two literal alternatives: each is its own implementation.
export function Alternatives(props: { flag: boolean }) {
  useThing(props.flag ? { onClose: () => undefined, label: "a" } : { onClose: () => 0, label: "b" });
  return <main />;
}

// Not written as an argument: the binding keeps ADR 0320's census.
export function BoundLiteral() {
  const options = { onClose: () => undefined, label: "a" };
  useThing(options);
  return <main />;
}

declare const extra: Partial<Options>;
declare function sink(value: unknown): void;
