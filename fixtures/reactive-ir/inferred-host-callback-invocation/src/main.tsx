import { Route, startClosed, after, inspect } from "reactive-package";
import { For, Show, Repeat, createEffect, onSettled, onCleanup, createRoot } from "solid-js";
import * as Solid from "solid-js";

function Invoke(props: { content: () => JSX.Element }) { return props.content(); }
function Ignore(_props: { content: () => JSX.Element }) { return null; }
function serverOnly(callback: () => void) {
  if (import.meta.env.SSR) callback();
}
const named = () => { startClosed(); /* exact local binding */ return <div />; };
function App() {
  return <div>
    <Route>{() => { startClosed(); /* authored Route child */ return <div />; }}</Route>
    <Invoke content={named} />
    <Ignore content={() => { startClosed(); /* ignored prop */ return <div />; }} />
    <button onClick={() => { startClosed(); /* feasible intrinsic event */ }} />
    <Show when={true}>{(_value) => { startClosed(); /* Show selected */ return <div />; }}</Show>
    <Show when={false}>{(_value) => { startClosed(); /* Show never selected */ return <div />; }}</Show>
    <Show when={false}><button onClick={() => { startClosed(); /* event under dead Show */ }} /></Show>
    <For each={[]}>{() => { startClosed(); /* empty For */ return <div />; }}</For>
    <Repeat count={0}>{(_index) => { startClosed(); /* empty Repeat */ return <div />; }}</Repeat>
    <Repeat count={1}>{(_index) => { startClosed(); /* Repeat selected */ return <div />; }}</Repeat>
    <For each={[1]}>{() => { startClosed(); /* feasible For child */ return <div />; }}</For>
  </div>;
}
App();
Route({children: () => { startClosed(); /* authored direct Route child */ return <div />; }});
const cancelled = setTimeout(() => { startClosed(); /* cancelled timer */ }, 0);
clearTimeout(cancelled);
setTimeout(() => { startClosed(); /* feasible timer */ }, 0);
queueMicrotask(() => { startClosed(); /* noncancelable microtask */ });
serverOnly(() => { startClosed(); /* server-only callback */ });
requestAnimationFrame(() => { startClosed(); /* feasible animation */ });
const cancelledFrame = requestAnimationFrame(() => { startClosed(); /* cancelled animation */ });
cancelAnimationFrame(cancelledFrame);
const listener = () => { startClosed(); /* feasible listener */ };
addEventListener("click", listener);
const removed = () => { startClosed(); /* removed listener */ };
addEventListener("click", removed);
removeEventListener("click", removed);
createRoot(() => {
  createEffect(() => 1, () => { startClosed(); /* feasible effect apply */ });
  onSettled(() => { startClosed(); /* feasible settlement */ });
  onCleanup(() => { startClosed(); /* feasible disposal */ });
  return undefined;
});
if (import.meta.env.SSR) setTimeout(() => { startClosed(); /* client-dead timer */ }, 0);
function rpc(callback: () => void) { "use server"; callback(); }
rpc(() => { startClosed(); /* RPC-only callback */ });
after(() => { startClosed(); /* authored deferred callback */ });
inspect(() => { startClosed(); /* non-Call callback row */ });
function shadowedTimer() {
  const setTimeout = (_callback: () => void, _delay: number) => 1;
  setTimeout(() => { startClosed(); /* shadowed timer */ }, 0);
}
shadowedTimer();
clearTimeout(setTimeout(() => { startClosed(); /* immediately consumed handle */ }, 0));
const inertCancelled = setTimeout(() => { startClosed(); /* inert-prefix cancellation */ }, 0);
const inertValue = 1;
clearTimeout(inertCancelled);
const guardedCancelled = setTimeout(() => { startClosed(); /* client-guarded cancellation */ }, 0);
if (!import.meta.env.SSR) clearTimeout(guardedCancelled);
const wrongHandle = setTimeout(() => { startClosed(); /* different handle stays feasible */ }, 0);
clearTimeout(123);
void wrongHandle; void inertValue;
onCleanup(() => { startClosed(); /* unregistered top-level cleanup */ });
Solid.createRoot(() => {
  Solid.createEffect(() => 1, {
    effect: () => { startClosed(); /* namespace effect bundle */ },
    error: () => { startClosed(); /* namespace error bundle */ }
  });
  return undefined;
});
const wrongCapture = () => { startClosed(); /* different capture stays feasible */ };
addEventListener("click", wrongCapture, true);
removeEventListener("click", wrongCapture, false);
const objectRemoved = () => { startClosed(); /* explicit capture object removed */ };
addEventListener("click", objectRemoved, { capture: false });
removeEventListener("click", objectRemoved, { capture: false });
function NeverSelected() {
  setTimeout(() => { startClosed(); /* nested component under dead Show */ }, 0);
  return <div/>;
}
function Selected() {
  setTimeout(() => { startClosed(); /* direct intrinsic child selected */ }, 0);
  return <div/>;
}
function IgnoreElement(_props: { children?: JSX.Element; content?: JSX.Element }) { return null; }
function NeverRead() {
  setTimeout(() => { startClosed(); /* nested component under ignored prop */ }, 0);
  return <div/>;
}
const deadSelection = <Show when={false}><NeverSelected/></Show>;
const ignoredElement = <IgnoreElement content={<NeverRead/>}/>;
const directSelection = <div><Selected/></div>;
void deadSelection; void ignoredElement; void directSelection;
function InvalidParameters({ x = (() => { throw new Error("not entered"); })() }: { x?: unknown }) {
  setTimeout(() => { startClosed(); /* JSX parameter initialization never completes */ }, 0);
  return <div/>;
}
setTimeout(() => {
  const invalidParameters = <InvalidParameters/>;
  void invalidParameters;
}, 0);
function registerWithoutOwner() {
  onCleanup(() => {
    setTimeout(() => { startClosed(); /* unregistered helper cleanup */ }, 0);
  });
}
registerWithoutOwner();
setTimeout(() => {
  onCleanup(() => {
    setTimeout(() => { startClosed(); /* unregistered fresh-stack cleanup */ }, 0);
  });
}, 0);
async function neverSettles() {
  setTimeout(() => { startClosed(); /* browser prefix before await */ }, 0);
  await new Promise<void>(() => {});
  setTimeout(() => { startClosed(); /* permanently pending continuation */ }, 0);
}
void neverSettles();
createRoot(() => {
  createEffect(() => 0, () => {
    setTimeout(() => { startClosed(); /* deferred constant effect never applies */ }, 0);
  }, { defer: true });
  createEffect(() => 0, () => {
    setTimeout(() => { startClosed(); /* constant effect initial apply */ }, 0);
  }, { defer: false });
  createEffect(() => 0, {
    effect: () => {},
    error: () => {
      setTimeout(() => { startClosed(); /* inert bundle never errors */ }, 0);
    }
  });
  createEffect(() => { throw new Error("error trigger"); }, {
    effect: () => {},
    error: () => {
      setTimeout(() => { startClosed(); /* throwing bundle error feasible */ }, 0);
    }
  });
});
function throwsDelay(): never { throw new Error("stop"); }
setTimeout(() => {
  setTimeout(() => { startClosed(); /* browser prefix before exact throw */ }, 0);
  (() => { throw new Error("abrupt exit"); })();
  setTimeout(() => { startClosed(); /* exact throwing IIFE dead continuation */ }, 0);
}, 0);
setTimeout(() => { startClosed(); /* argument never registers timer */ }, throwsDelay());
