import { createReaction, createSignal, onCleanup } from "solid-js";
import { keep, wrap } from "./helpers";

const [count] = createSignal(0);

// A JavaScript wrapper that is never written returns its callback: the literal
// is the callback, and its cleanup runs with no owner.
const trackKept = createReaction(keep(() => onCleanup(() => {})));
trackKept(() => count());

// `wrap` is written in its module (`wrap = function …`), so the call does not
// return its parameter, whatever the declaration said: the literal is not named.
const trackWrapped = createReaction(wrap(() => onCleanup(() => {})));
trackWrapped(() => count());
