import { onCleanup, untrack, onSettled, getOwner, runWithOwner } from "solid-js";
function handlers(cb: () => void) {return {onClick: () => cb()};}
export function Untracked() {
  const h=handlers(() => untrack(() => onCleanup(() => undefined)));
  return <button {...h}>untrack</button>;
}
export function Async() {
  const h=handlers(async () => {onCleanup(() => undefined); await Promise.resolve(); onCleanup(() => undefined);});
  return <button {...h}>async</button>;
}
export function Conditional() {
  const yes = true;
  const a = () => onCleanup(() => undefined);
  const h=handlers(yes ? a : () => undefined);
  return <button {...h}>conditional</button>;
}
export function Settled() {
  const h=handlers(() => onSettled(() => undefined));
  return <button {...h}>settled</button>;
}
function Kids(props: {children: () => void}) {return <button onClick={() => props.children()}>kid</button>;}
export function Child() {return <Kids>{() => onCleanup(() => undefined)}</Kids>;}
