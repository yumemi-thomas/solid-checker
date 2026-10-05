import { createSignal } from "solid-js";
import {
  CallsDuringRender, Chained, Direct, Keeps, Loop, MergeEscapes, Merged, Mixed, OmitSpread, Spreads,
  SpreadsRender, SpreadsToCaller, Wrapped,
} from "./buttons";

// ---- Clean: the literal reaches only an intrinsic element's event handler,
// so its read runs on event dispatch, never in the caller's strict-read window.

export function UsesDirect() {
  const [count] = createSignal(0);
  return <Direct onPress={() => console.log(count())} label="a" />;
}
export function UsesWrapped() {
  const [count] = createSignal(0);
  return <Wrapped onPress={() => console.log(count())} label="a" />;
}
export function UsesChained() {
  const [count] = createSignal(0);
  return <Chained onPress={() => console.log(count())} label="a" />;
}
export function UsesLocal() {
  const [count] = createSignal(0);
  return <LocalDirect onPress={() => console.log(count())} />;
}
export function UsesSpreads() {
  const [count] = createSignal(0);
  return <Spreads onPress={() => console.log(count())} label="a" />;
}
export function UsesMerged() {
  const [count] = createSignal(0);
  return <Merged onPress={() => console.log(count())} />;
}
export function UsesOmitSpread() {
  const [count] = createSignal(0);
  return <OmitSpread onPress={() => console.log(count())} label="a" />;
}
function LocalDirect(props: { onPress?: (event: unknown) => void }) {
  return <button onClick={props.onPress}>local</button>;
}

// ---- Uncertifiable: nothing proves the literal runs only from an event.

export function UsesCallsDuringRender() {
  const [count] = createSignal(0);
  return <CallsDuringRender onPress={() => console.log(count())} label="a" />;
}
export function UsesSpreadsToCaller() {
  const [count] = createSignal(0);
  return <SpreadsToCaller onPress={() => console.log(count())} label="a" />;
}
export function UsesMergeEscapes() {
  const [count] = createSignal(0);
  return <MergeEscapes onPress={() => console.log(count())} label="a" />;
}
export function UsesSpreadsRender() {
  const [count] = createSignal(0);
  return <SpreadsRender render={() => console.log(count())} />;
}
export function UsesKeeps() {
  const [count] = createSignal(0);
  return <Keeps onPress={() => console.log(count())} label="a" />;
}
export function UsesMixed() {
  const [count] = createSignal(0);
  return <Mixed onPress={() => console.log(count())} label="a" />;
}
export function UsesLoop() {
  const [count] = createSignal(0);
  return <Loop onPress={() => console.log(count())} label="a" />;
}

