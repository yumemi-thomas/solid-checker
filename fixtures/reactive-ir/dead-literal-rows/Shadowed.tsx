import { createContext, useContext } from "solid-js";

const Bare = createContext<number>();

// A project component named like the flow components is no flow component: nothing proves its children never run, and it calls them.
function Repeat(props: { count: number; children: () => JSX.Element }) {
  return <div>{props.children()}</div>;
}
function Show(props: { when: boolean; children: JSX.Element }) {
  return <div>{props.children}</div>;
}

export function ShadowedRepeatZero() {
  return <Repeat count={0}>{() => <p>{(() => { useContext(Bare); return 1; })()}</p>}</Repeat>;
}
export function ShadowedShowFalse() {
  return <Show when={false}><p>{(() => { useContext(Bare); return 1; })()}</p></Show>;
}
