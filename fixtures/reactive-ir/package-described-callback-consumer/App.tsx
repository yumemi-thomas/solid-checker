import { createSignal } from "solid-js";
import { pipe, pipeOpen } from "reactive-package";

const [count] = createSignal(0);

// `pipe`'s accepted contract says its call runs neither argument and keeps
// both only in the function it returns (`result-access`), and that one call
// of that function runs each of them exactly once, on its caller's stack
// (ADR 0152). Called in the component body, the first argument's read of
// `count` happens there, untracked: reported where it is written.
export function UntrackedPipe() {
  const measure = pipe((raw: string) => raw.length + count(), (length: number) => length);
  const value = measure("text");
  return <div>{value}</div>;
}

// The same function called inside JSX runs the same read tracked, and
// nothing is reported.
export function TrackedPipe() {
  const measure = pipe((raw: string) => raw.length + count(), (length: number) => length);
  return <div>{measure("text")}</div>;
}

// Never called, the returned function runs neither argument: nothing places
// the read anywhere, and nothing is reported.
export function UnusedPipe() {
  pipe((raw: string) => raw.length + count(), (length: number) => length);
  return <div />;
}

// The control: the same declaration and the same use with `callbacks` left
// open, so the import reports the open claim and the read inside the callback
// is of unproven timing, never a proven untracked read.
export function OpenPipe() {
  const measure = pipeOpen((raw: string) => raw.length + count(), (length: number) => length);
  const value = measure("text");
  return <div>{value}</div>;
}
