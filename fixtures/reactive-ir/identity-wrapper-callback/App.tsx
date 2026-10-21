import { createEffect, createReaction, createSignal, onCleanup } from "solid-js";
import { wrapImported } from "./helpers";

const [count] = createSignal(0);

// A declared wrapper returns its callback on its only path.
function wrapReaction(callback: () => void): () => void {
  return callback;
}
const trackDeclared = createReaction(wrapReaction(() => onCleanup(() => {})));
trackDeclared(() => count());

// An arrow wrapper: its expression body is its parameter.
const wrapArrow = (callback: () => void): (() => void) => callback;
const trackArrow = createReaction(wrapArrow(() => onCleanup(() => {})));
trackArrow(() => count());

// An imported wrapper, resolved through the import.
const trackImported = createReaction(wrapImported(() => onCleanup(() => {})));
trackImported(() => count());

// A wrapper's return is the handler, so the handler runs with no owner.
export function Button() {
  return (
    <button
      onClick={wrapReaction(() => {
        createEffect(count, () => {});
      })}
    />
  );
}
