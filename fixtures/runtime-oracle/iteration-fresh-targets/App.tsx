import { createSignal } from "solid-js";
import { put } from "./put";

// ADR 1040: the window goes only to seen code (a listener that reaches nothing, a timer's arrow, `clearTimeout`), so an
// open write of a project helper is no write to an intrinsic it was handed (b); `next` and `return` defined on an array
// the intact global `Array` made (a), and on a `let` only ever bound to literals and on `Object.create(null)` (c, ADR
// 1024's made objects), reach no iterator. The slots of `createSignal` stay exact.
window.addEventListener("resize", () => console.log("resized"));
const timer = window.setTimeout(() => console.log("later"), 0);
window.clearTimeout(timer);
put({}, "a", 1);

const out = new Array<number>(2);
Object.defineProperty(out, "next", { value: 1 });
const made = Array(2);
Object.defineProperty(made, "return", { value: 1 });
const from = Array.from({ length: 2 }, (_, i) => i);
Object.defineProperty(from, "next", { value: 1 });

let holder: object = {};
if (document.title === "") holder = [];
Object.defineProperty(holder, "next", { value: 1 });
const bare = Object.create(null);
bare.next = 1;

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
