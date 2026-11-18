import { createMemo, refresh } from "solid-js";
import * as S from "solid-js";

// An accessor no code of this program reads: whatever it does with its argument is unknown.
declare function keep(value: unknown): void;

// Proved: `refresh` reads the property the runtime set on the accessor and never calls it or hands it on.
export function Direct() {
  const m1 = createMemo(() => 1);
  return <button onClick={() => { refresh(m1); }}>go</button>;
}

// Proved: the same primitive through the namespace member.
export function Namespace() {
  const m2 = createMemo(() => 1);
  return <button onClick={() => { S.refresh(m2); }}>go</button>;
}

// Open: the accessor also goes to a function nothing here reads.
export function AlsoHanded() {
  const m3 = createMemo(() => 1);
  return <button onClick={() => { refresh(m3); keep(m3); }}>go</button>;
}

// Misuse twin: `refresh` is a write-like invalidation, and in a component body it writes in an owned scope.
export function InBody() {
  const m4 = createMemo(() => 1);
  refresh(m4);
  return <div />;
}
