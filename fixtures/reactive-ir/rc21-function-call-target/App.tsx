import { createStore } from "solid-js";

const [st] = createStore([2, 1]);
const plain = [2, 1];

function own(this: unknown, value: unknown) { return value; }

export function App() {
  return (
    <p onClick={() => {
      // No fact gives `.call`'s target: the store reaches code nothing reads.
      Array.prototype.sort.call(st);
      Array.prototype.reverse.apply(st);
      // Neither is a reactive value through an unknown target, or an unknown target.
      Array.prototype.sort.call(plain);
      own.call(null, st);
    }}>x</p>
  );
}
