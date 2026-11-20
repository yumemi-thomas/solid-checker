import { affects, createMemo } from "solid-js";
import * as S from "solid-js";

// An accessor no code of this program reads: whatever it does with its argument is unknown.
declare function keep(value: unknown): void;

// Proved: `affects` reads `$TARGET` and `$REFRESH` off the accessor, marks the node the second names and never calls the accessor or
// hands it on (`@solidjs/signals` 2.0.0-rc.21, `dist/dev.js:10018-10050`). Nothing in this program may write a prototype, so the first
// read, which walks `Function.prototype`, runs no code.
export function Direct() {
  const m1 = createMemo(() => 1);
  return <button onClick={() => { affects(m1); }}>go</button>;
}

// Proved: the same primitive through the namespace member.
export function Namespace() {
  const m2 = createMemo(() => 1);
  return <button onClick={() => { S.affects(m2); }}>go</button>;
}

// Open: the accessor also goes to a function nothing here reads.
export function AlsoHanded() {
  const m3 = createMemo(() => 1);
  return <button onClick={() => { affects(m3); keep(m3); }}>go</button>;
}

// Proved: a project function that takes the accessor and hands it to `affects` (the walk follows the parameter to that call).
function mark(target: () => unknown) {
  affects(target);
}
export function Wrapper() {
  const m5 = createMemo(() => 1);
  return <button onClick={() => { mark(m5); }}>go</button>;
}

// Open: a local that shares the name is no Solid primitive; what it does with the accessor is the project's, here unread.
export function Shadowed() {
  const m6 = createMemo(() => 1);
  const affects = (target: unknown) => { keep(target); };
  return <button onClick={() => { affects(m6); }}>go</button>;
}

// Proved: `affects` has no owned-scope guard (it declares and schedules, `dist/dev.js:10018-10050`), so the call in a component body is no
// write of a signal; Chrome is quiet. (`refresh` is the one that writes there: ADR 1089's `InBody` stays a violation.)
export function InBody() {
  const m4 = createMemo(() => 1);
  affects(m4);
  return <div />;
}
