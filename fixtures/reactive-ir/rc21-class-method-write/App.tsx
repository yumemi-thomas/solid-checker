import { createStore } from "solid-js";

class Counter {
  n = 0;
  inc() {
    this.n++;
  }
  get() {
    return 1;
  }
}

// The method runs with the read proxy as `this`: the write is dropped.
export function Method() {
  const [store] = createStore({ counter: new Counter() });
  return <button onClick={() => store.counter.inc()}>x</button>;
}

// In the setter the draft is `this`; a method that writes nothing is no write.
export function Fine() {
  const [store, setStore] = createStore({ counter: new Counter() });
  return (
    <button
      onClick={() => {
        setStore((draft) => {
          draft.counter.inc();
        });
        store.counter.get();
      }}
    >
      x
    </button>
  );
}

// `Reflect.deleteProperty` is a write through the proxy.
export function Deleted() {
  const [store] = createStore<{ a?: number }>({ a: 1 });
  return <button onClick={() => Reflect.deleteProperty(store, "a")}>x</button>;
}
