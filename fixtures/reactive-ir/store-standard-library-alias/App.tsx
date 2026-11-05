import { createStore } from "solid-js";

const [s] = createStore({ a: 1 });
const plain = { a: 1 };
const assign = Object.assign;
const set = Reflect.set;
const notAssign = (target: object, value: object) => ({ target, value });

export function Handlers() {
  return (
    <>
      {/* Violations: the alias's declaration is the standard library's write. */}
      <button onClick={() => assign(s, { a: 2 })}>assign</button>
      <button onClick={() => set(s, "a", 2)}>set</button>
      {/* Silent: a project function, and a plain target. */}
      <button onClick={() => notAssign(s, { a: 2 })}>project</button>
      <button onClick={() => assign(plain, { a: 2 })}>plain</button>
    </>
  );
}
