import { For, Match, Repeat, Repeat as Rep, Show, Switch, createContext, useContext } from "solid-js";
import * as S from "solid-js";

const Bare = createContext<number>();

// ---- Dead: a LITERAL prop keeps the children from ever running; the read is U (unreached-dead-code), never a violation ----

export function RepeatZeroTracked() {
  return <div><Repeat count={0}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat></div>;
}
export function ForEmptyTracked() {
  return <div><For each={[]}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</For></div>;
}
export function ShowFalseTracked() {
  return <div><Show when={false}><p>{(() => { useContext(Bare); return 1; })()}</p></Show></div>;
}
export function MatchFalseTracked() {
  return <div><Switch><Match when={false}><p>{(() => { useContext(Bare); return 1; })()}</p></Match></Switch></div>;
}
// `Repeat` reads `props.children` only inside a row, so a child that is no function is dead as well.
export function RepeatZeroElementChild() {
  return <div><Repeat count={0}><p>{(() => { useContext(Bare); return 1; })()}</p></Repeat></div>;
}
// The tag is the declaration, not the spelling: an alias and a namespace member.
export function RepeatZeroAlias() {
  return <div><Rep count={0}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Rep></div>;
}
export function ForEmptyNamespace() {
  return <div><S.For each={[]}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</S.For></div>;
}
// The literal under parentheses and a TypeScript assertion.
export function RepeatZeroParenthesized() {
  return <div><Repeat count={(0)}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat></div>;
}
export function ShowFalseAsserted() {
  return <div><Show when={false as boolean}><p>{(() => { useContext(Bare); return 1; })()}</p></Show></div>;
}
// A live row nested in a dead one is dead.
export function ForNestedInDeadRepeat() {
  return <div><Repeat count={0}>{() => <For each={[1]}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</For>}</Repeat></div>;
}
