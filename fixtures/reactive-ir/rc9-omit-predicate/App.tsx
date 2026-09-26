// `omit(props, hidden)` on @solidjs/signals@2.0.0-rc.9: a single function
// argument is a key predicate (`keys.length === 1 && typeof keys[0] ===
// "function"`, `dist/dev.js:4380`). Wherever `Proxy` exists, `omit` only
// stores it; the view's traps call it on every read of the returned object
// (`isHidden`, `:3495-3498`; `omitTraps`, `:4178-4241`), in the reader's
// tracking and ownership. The call site does not decide that scope, so code in
// a predicate is never a violation, and a predicate not proven inert is
// uncertifiable (SC9012) at the predicate argument.
import { createSignal, omit, untrack } from "solid-js";
import { hiddenKey } from "./keys";

const [mode, setMode] = createSignal("a");

type Props = { a: string; b: string };

// Positive (uncertifiable): the predicate reads `mode()` whenever the view is
// read. Here that is tracked JSX, but the call site cannot know it; a strict
// read claim would be wrong and silence would certify it.
export function InlinePredicateRead(props: Props) {
  const rest = omit(props, (key) => key === mode());
  return <div title={rest.b} />;
}

// Positive (uncertifiable): the write runs under whatever owner reads the
// view, which the call site does not decide; no write-scope claim either way.
export function InlinePredicateWrite(props: Props) {
  const rest = omit(props, (key) => {
    setMode("b");
    return key === "a";
  });
  return <div title={rest.b} />;
}

// Positive (uncertifiable): the same predicate passed by name.
export function NamedPredicateRead(props: Props) {
  function hidden(key: string | symbol) {
    return key === mode();
  }
  const rest = omit(props, hidden);
  return <div title={rest.b} />;
}

// Positive control: the component body itself is still analysed. A direct read
// here runs once in the body's strict-read window and never updates. The
// predicate beside it reads nothing reactive, so it is silent.
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

// Negative: an inert predicate whose only call is a standard-library method.
export function StandardLibraryPredicate(props: Props) {
  const rest = omit(props, (key) => typeof key === "string" && key.startsWith("_"));
  return <div title={rest.b} />;
}

// Positive (uncertifiable): a `const` arrow named at the position.
export function ConstPredicateRead(props: Props) {
  const hidden = (key: string | symbol) => key === mode();
  const rest = omit(props, hidden);
  return <div title={rest.b} />;
}

// Positive (uncertifiable): the predicate reads the props object itself.
export function PropsPredicateRead(props: Props) {
  const rest = omit(props, (key) => key === props.a);
  return <div title={rest.b} />;
}

// Positive (uncertifiable, fail-closed): the predicate calls a project helper.
// `isUnderscored` happens to be inert, but the rule does not follow calls out
// of the predicate, so it cannot prove that.
function isUnderscored(key: string | symbol) {
  return key === "_a";
}
export function HelperPredicate(props: Props) {
  const rest = omit(props, (key) => isUnderscored(key));
  return <div title={rest.b} />;
}

// Positive (uncertifiable): the predicate is imported, so no body is
// inspectable at this call.
export function ImportedPredicate(props: Props) {
  const rest = omit(props, hiddenKey);
  return <div title={rest.b} />;
}

// A forwarded parameter's body is its caller's, so nothing is claimed at this
// `omit`. The exported wrapper is still uncertifiable for callers outside the
// project (the unknown-callback obligation; contract generation opens its
// `callbacks`, see `fixtures/package-contracts/rc9-callback-forms`).
export function hideBy(props: Props, hidden: (key: string | symbol) => boolean) {
  return omit(props, hidden);
}

// Positive (uncertifiable): an in-project call of the wrapper hands `omit` a
// predicate that reads `mode()`. It runs on reads of `rest`, not in this body,
// so it is not a strict read of the component either.
export function ForwardedPredicateCaller(props: Props) {
  const rest = hideBy(props, (key) => key === mode());
  return <div title={rest.b} />;
}
