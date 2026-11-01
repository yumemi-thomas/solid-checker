import { createStore } from "solid-js";

// Positive: a standard-library write of the store's own proxy, and a
// destructuring assignment whose leaf is a store member, are dropped outside
// the store's setter (rc.17 warns STORE_WRITE_OUTSIDE_SETTER at each). The
// destructuring leaf is a write, not a read: no strict-read finding.
export function StandardLibraryWrites() {
  const [store, setStore] = createStore({ v: 0, w: 0 });
  Object.assign(store, { v: 7 });
  Reflect.set(store, "v", 7);
  Object.defineProperty(store, "v", { value: 7 });
  ({ v: store.v } = { v: 7 });
  [store.w] = [7];
  return (
    <button
      onClick={() => {
        Object.assign(store, { v: 8 });
        Reflect.set(store, "v", 8);
        Object.defineProperty(store, "v", { value: 8 });
        ({ v: store.v } = { v: 8 });
        setStore((draft) => {
          draft.w = 8;
        });
      }}
    >
      {store.v}
    </button>
  );
}

// Positive: another store's setter does not write-enable this proxy.
export function OtherSetter() {
  const [store] = createStore({ v: 0 });
  const [other, setOther] = createStore({ n: 0 });
  return (
    <button
      onClick={() =>
        setOther(() => {
          Object.assign(store, { v: 7 });
          ({ v: store.v } = { v: 7 });
        })
      }
    >
      {other.n}
    </button>
  );
}

// Uncertifiable: the target is a link past the root, which no fact proves the
// proxy wraps (ADR 0465, 0476). In a handler, so the link's Get is no strict
// read.
export function PastRoot() {
  const [store] = createStore({ o: { x: 0 } });
  return (
    <button
      onClick={() => {
        Object.assign(store.o, { x: 7 });
      }}
    >
      {store.o.x}
    </button>
  );
}

// Negative: inside the store's own setter the original proxy is write-enabled
// (rc.17 commits each, read back after `flush()`), and the draft is the
// setter's own.
export function OwnSetter() {
  const [store, setStore] = createStore({ v: 0 });
  return (
    <button
      onClick={() => {
        setStore(() => {
          Object.assign(store, { v: 1 });
          Reflect.set(store, "v", 2);
          Object.defineProperty(store, "v", { value: 3 });
          ({ v: store.v } = { v: 4 });
        });
        setStore((draft) => {
          Object.assign(draft, { v: 5 });
          Reflect.set(draft, "v", 6);
          Object.defineProperty(draft, "v", { value: 7 });
          ({ v: draft.v } = { v: 8 });
        });
      }}
    >
      {store.v}
    </button>
  );
}

// Negative: the store is a source, not the target (a copy), and a write to a
// copy lands. In a handler, so the copies' Gets are no strict reads.
export function Copies() {
  const [store] = createStore({ v: 0 });
  return (
    <button
      onClick={() => {
        const copy = Object.assign({}, store, { v: 7 });
        Object.assign(copy, { v: 7 });
        Reflect.set({ ...store }, "v", 7);
        Object.defineProperty({ ...store }, "v", { value: 7 });
        ({ ...store }).v = 7;
        ({ v: copy.v } = { v: 7 });
      }}
    >
      {store.v}
    </button>
  );
}

// Negative: the callee is not the standard library's: a project object named
// `Object` or `Reflect` shadows the global, and a project `assign` is a
// project function. (Handing the store to a member callee the analysis cannot
// resolve is another rule's obligation.)
export function NotTheLibrary() {
  const [store] = createStore({ v: 0 });
  const Object = { assign: (target: object, _source: object) => target };
  const Reflect = { set: (_target: object, _key: string, _value: unknown) => true };
  const helpers = { assign: (target: object, _source: object) => target };
  Object.assign(store, { v: 7 });
  Reflect.set(store, "v", 7);
  helpers.assign(store, { v: 7 });
  return <p>{store.v}</p>;
}

// Negative: a shadowing parameter of a plain function is not the store.
export function shadowed(store: { v: number }) {
  Object.assign(store, { v: 7 });
  ({ v: store.v } = { v: 7 });
}
