import { For, Match, Repeat, Repeat as Rep, Show, Switch, createContext, useContext } from "solid-js";
import * as S from "solid-js";

const Bare = createContext<number>();

// The read the cases share: SC4002 (no provider on the owner chain) wherever it runs.

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

// ---- Live: the same read where the children may run stays a violation ----

export function RepeatOneTracked() {
  return <div><Repeat count={1}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat></div>;
}
export function ForOneTracked() {
  return <div><For each={[1]}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</For></div>;
}
export function ShowTrueTracked() {
  return <div><Show when={true}><p>{(() => { useContext(Bare); return 1; })()}</p></Show></div>;
}
// The fallback of an empty `For`, a zero `Repeat` and a false `Show` runs exactly when their children do not.
export function ForEmptyFallbackTracked() {
  return <div><For each={[]} fallback={<p>{(() => { useContext(Bare); return 1; })()}</p>}>{() => <i>row</i>}</For></div>;
}
export function RepeatZeroFallbackTracked() {
  return <div><Repeat count={0} fallback={<p>{(() => { useContext(Bare); return 1; })()}</p>}>{() => <i>row</i>}</Repeat></div>;
}
export function ShowFalseFallbackTracked() {
  return <div><Show when={false} fallback={<p>{(() => { useContext(Bare); return 1; })()}</p>}><i>child</i></Show></div>;
}
// `For` reads `props.children` once when it maps: an expression that makes the row function is evaluated, only the row's body is dead.
function makeRow() {
  useContext(Bare);
  return () => <i>row</i>;
}
export function ForEmptyRowMadeByCall() {
  return <div><For each={[]}>{makeRow()}</For></div>;
}

// ---- Not literal: anything but the literal keeps today's reading, whatever the value is at run time ----

const zero = 0;
const none: never[] = [];
export function RepeatNamedZero() {
  return <div><Repeat count={zero}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat></div>;
}
export function ForNamedEmpty() {
  return <div><For each={none}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</For></div>;
}
export function ShowNegated() {
  return <div><Show when={!true}><p>{(() => { useContext(Bare); return 1; })()}</p></Show></div>;
}
// A spread may replace the prop.
export function RepeatZeroWithSpread(props: { rest?: { from?: number } }) {
  return <div><Repeat count={0} {...props.rest}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat></div>;
}
