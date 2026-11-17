import { For, Match, Repeat, Repeat as Rep, Show, Switch, createContext, useContext } from "solid-js";
import * as S from "solid-js";

const Bare = createContext<number>();

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
