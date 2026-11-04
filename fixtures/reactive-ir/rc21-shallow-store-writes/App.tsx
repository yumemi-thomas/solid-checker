import { createStore } from "solid-js";

// A shallow store serves values raw: a write below the root lands there, with
// no guard and no warning (ADR 0626).
export function Raw() {
  const [state] = createStore({ list: [{ a: 1 }], row: { a: 1 } }, { shallow: true });
  const click = () => {
    state.list[0].a = 2;
    state.row.a = 2;
    state.list.push({ a: 3 });
  };
  return <button onClick={click}>x</button>;
}

// Its root keys are the proxy's: the write is dropped.
export function Root() {
  const [state] = createStore({ row: { a: 1 } }, { shallow: true });
  const click = () => {
    state.row = { a: 2 };
  };
  return <button onClick={click}>x</button>;
}

// A deep store, and options this does not prove shallow, keep their answers.
export function Deep(props: { options: { shallow?: boolean } }) {
  const [deep] = createStore({ row: { a: 1 } });
  const [off] = createStore({ row: { a: 1 } }, { shallow: false });
  const [unknown] = createStore({ row: { a: 1 } }, props.options);
  const click = () => {
    deep.row.a = 2;
    off.row.a = 2;
    unknown.row.a = 2;
  };
  return <button onClick={click}>x</button>;
}
