import { createEffect, createSignal } from "solid-js";
import * as Health from "./health";

// ADR 1056: `run` is a parameter of an arrow written as the `schedule` member of an object literal handed whole to the exact project
// function `createHealth` (through a namespace import); `createHealth` only reads members of its parameter, calls `schedule` with an
// arrow, and calls `now`, an arrow, with the literal as `this`. The timer calls an arrow, the window reaches no code, the slots of every
// `createSignal` stay exact, and the reads and the write below are the defects they look like (kui shell's shapes).

// STRICT_READ_UNTRACKED inside `createHealth`, called from this body: it reads the signal it made (kui `App.tsx`'s `createHealth({…})`).
export function Shell() {
  const health = Health.createHealth({
    now: () => Date.now(),
    schedule: (delayMs, run) => void setTimeout(run, delayMs),
  });
  return <time>{health()}</time>;
}

// STRICT_READ_UNTRACKED: a prop read in a component body into a signal's seed (kui `TopicListPage.tsx`).
function Search(props: { query: { search: string } }) {
  const [typed, setTyped] = createSignal(props.query.search);
  // STRICT_READ_UNTRACKED: a signal read in an effect's apply callback.
  createEffect(
    () => props.query.search,
    (search: string) => {
      if (search !== typed().trim()) setTyped(search);
    },
  );
  return <p>{typed()}</p>;
}

export function SearchHost() {
  const [query, setQuery] = createSignal({ search: "a" });
  return (
    <div>
      <Search query={query()} />
      <button onClick={() => setQuery({ search: "b" })}>next</button>
    </div>
  );
}

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
