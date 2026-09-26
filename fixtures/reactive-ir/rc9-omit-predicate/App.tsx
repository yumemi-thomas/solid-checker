// `omit(props, hidden)` on @solidjs/signals@2.0.0-rc.9: a single function
// argument is a key predicate (`keys.length === 1 && typeof keys[0] ===
// "function"`, `dist/dev.js:4380`). Wherever `Proxy` exists, `omit` only
// stores it; the view's traps call it on every read of the returned object
// (`isHidden`, `:3495-3498`; `omitTraps`, `:4178-4241`), in the reader's
// tracking and ownership. Nothing here follows those reads, so code inside a
// predicate is claimed nowhere -- in particular not in the component body it
// is written in.
import { createSignal, omit, untrack } from "solid-js";

const [mode, setMode] = createSignal("a");

type Props = { a: string; b: string };

// Negative: the predicate reads `mode()` whenever the view is read -- here in
// tracked JSX, where the read subscribes. Treated as a component-body read, it
// used to be reported as a strict read that never updates.
export function InlinePredicateRead(props: Props) {
  const rest = omit(props, (key) => key === mode());
  return <div title={rest.b} />;
}

// Negative: the write runs under whatever owner reads the view, which the
// call site does not decide; no write-scope claim is made for it.
export function InlinePredicateWrite(props: Props) {
  const rest = omit(props, (key) => {
    setMode("b");
    return key === "a";
  });
  return <div title={rest.b} />;
}

// Negative: the same predicate passed by name.
export function NamedPredicateRead(props: Props) {
  function hidden(key: string | symbol) {
    return key === mode();
  }
  const rest = omit(props, hidden);
  return <div title={rest.b} />;
}

// Positive control: the component body itself is still analysed. A direct read
// here runs once in the body's strict-read window and never updates.
export function BodyReadBesidePredicate(props: Props) {
  const rest = omit(props, (key) => key === "a");
  const current = mode();
  return <div title={rest.b}>{current}</div>;
}

// Control: the key-list form has no callback; a key list is a value.
export function KeyListForm(props: Props) {
  const rest = omit(props, "a");
  return <div title={untrack(() => rest.b)} />;
}
