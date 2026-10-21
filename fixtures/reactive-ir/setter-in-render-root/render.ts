import * as web from "@solidjs/web";
import { createEffect, createSignal } from "solid-js";
import { hydrate, render } from "@solidjs/web";

const [count, setCount] = createSignal(0);
const [done, setDone] = createSignal(false);

// `render` calls its code inside the root it creates (@solidjs/web rc.13
// dist/web.dev.js:488-495): a setter directly in the code callback is SC2001.
render(() => {
  setCount(1);
  return document.createElement("span");
}, document.createElement("div"));

// `hydrate` reaches that `render` on every path (dist/web.dev.js:1521-1620).
hydrate(() => {
  setCount(2);
  return document.createElement("span");
}, document.createElement("div"));

// A helper only the code callback calls runs under the same root.
function writeCount() {
  setCount(3);
}
render(() => {
  writeCount();
  return document.createElement("span");
}, document.createElement("div"));

// Namespace spelling of the same export.
web.render(() => {
  setCount(4);
  return document.createElement("span");
}, document.createElement("div"));

// Uncertifiable, not reported: a listener created in the code callback runs
// under the root if a synchronous dispatch fires it there, and nothing here
// proves that either way.
render(() => {
  const span = document.createElement("span");
  span.addEventListener("click", () => setCount(5));
  return span;
}, document.createElement("div"));

// Negative: a createEffect apply callback is a legal write region.
render(() => {
  createEffect(
    () => count(),
    () => {
      setDone(true);
    },
  );
  return document.createElement("span");
}, document.createElement("div"));

// Negative: a local `render` shadows the import, so the call is not
// @solidjs/web's and its callback runs at module scope, outside any root.
{
  const render = (code: () => unknown) => {
    code();
  };
  render(() => {
    setCount(6);
  });
}
