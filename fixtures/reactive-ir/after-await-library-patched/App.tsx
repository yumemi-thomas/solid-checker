import { createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const pause = () => Promise.resolve();
void count;

// The calls of `after-await-library`'s `Outside`, in a program that writes a built-in: `String.prototype.concat` is the project's function now. One written built-in anywhere takes the
// proof from every standard call, construct and setter, because a standard call reaches other built-ins (`Promise.all("ab")` calls `Promise.resolve`, `new Set("ab")` calls `add`,
// `"abc".match("a")` looks up `Symbol.match`), so each call below keeps its row (Research B's `aa-promise-all-resolve`, `aa-set-add`, `aa-math-assign`, `aa-concat-assign`).
String.prototype.concat = function () {
  return String(this);
};

export async function Outside() {
  await pause();
  JSON.stringify({ n: 1 });
  Promise.resolve(1);
  setCount(1);
  new Date();
  Math.max(1, 2);
  Object.keys({ a: 1 });
  setTimeout(() => {
    count();
  }, 0);
}
// The string form of a timer is code the host evaluates: a program that writes one has no inert standard call either (the census takes it as code obtained as a value).
export async function Evaluated() {
  await pause();
  setTimeout("count()", 0);
}
