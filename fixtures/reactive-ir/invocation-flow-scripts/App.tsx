import { createSignal } from "solid-js";
import { fireModule, relayModule, relayThroughGlobal } from "./module-lib";
import { fireCommon, relayCommon } from "./cjs-lib.js";

// Each callback's `now` closure runs in the component body: all
// uncertifiable.

// `fireGlobal` lives in a script: its census in that file sees no caller.
export function ScriptGlobal() {
  const [count] = createSignal(0);
  const handlers = relayGlobal(() => console.log(count()));
  fireGlobal(handlers.now);
  return <button onClick={handlers.onClick}>s</button>;
}

export function ModuleControl() {
  const [count] = createSignal(0);
  const handlers = relayModule(() => console.log(count()));
  fireModule(handlers.now);
  return <button onClick={handlers.onClick}>m</button>;
}

export function GlobalThisRelay() {
  const [count] = createSignal(0);
  const handlers = relayThroughGlobal(() => console.log(count()));
  (globalThis as unknown as { fireStored?: () => void }).fireStored?.();
  return <button onClick={handlers.onClick}>g</button>;
}

export function CommonJsExport() {
  const [count] = createSignal(0);
  const handlers = relayCommon(() => console.log(count()));
  fireCommon(handlers.now);
  return <button onClick={handlers.onClick}>c</button>;
}
