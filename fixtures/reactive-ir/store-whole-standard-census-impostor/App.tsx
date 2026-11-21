import { createStore } from "solid-js";

// ADR 1178's premise (ADRs 0840, 1129): the program replaces `Object.values` and the array iterator, so the standard call and the
// iteration may run project code with the store as an argument or `this`, where no row stands. Each stays uncertifiable.
Object.values = ((value: object) => [value]) as typeof Object.values;
Array.prototype[Symbol.iterator] = function* (this: unknown[]) {
  yield this.length;
} as typeof Array.prototype.values;

export function Patched() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([1, 2]);
  const click = () => {
    Object.values(row);
    void [...list];
  };
  return <button onClick={click} />;
}
