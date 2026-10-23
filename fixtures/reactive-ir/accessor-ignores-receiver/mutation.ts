import { createMemo, createSignal, createStore, type Accessor } from "solid-js";

export type State = { readonly kind: "idle" | "running" | "done" };

// The `createMutation` shape (kui `kernel/src/data/mutation.ts`): `call` runs
// in the returned async `run`, before its first await. `state` is the
// accessor `createSignal` returned, `busy` an arrow, `reset` an arrow.
export function createMutation<A extends unknown[]>(call: (...args: A) => Promise<number>) {
  const [state, setState] = createSignal<State>({ kind: "idle" }, { ownedWrite: true });
  let running = false;
  const run = async (...args: A) => {
    if (running) return 0;
    running = true;
    setState({ kind: "running" });
    const answer = await call(...args);
    running = false;
    setState(() => ({ kind: "done" }));
    return answer;
  };
  return {
    state,
    run,
    reset: () => {
      if (!running) setState({ kind: "idle" });
    },
    busy: () => state().kind === "running",
  };
}

// `call` runs after `run`'s first await: a continuation no invocation places.
export function createLateMutation(call: () => Promise<number>) {
  const [state] = createSignal<State>({ kind: "idle" });
  const run = async () => {
    await Promise.resolve();
    return await call();
  };
  return { state, run };
}

// The setter itself as a member, and a `createMemo` accessor.
export function createMemoMutation(call: () => Promise<number>) {
  const [state, setState] = createSignal<State>({ kind: "idle" }, { ownedWrite: true });
  const busy = createMemo(() => state().kind === "running");
  const run = async () => {
    setState({ kind: "running" });
    return await call();
  };
  return { set: setState, busy, run };
}

// A store's setter as a member.
export function createStoreMutation(call: () => Promise<number>) {
  const [store, setStore] = createStore({ kind: "idle" as State["kind"] });
  const run = async () => {
    setStore((draft) => {
      draft.kind = "running";
    });
    return await call();
  };
  return { store, set: setStore, run };
}

// An arrow wrapping the accessor: an arrow has no `this` of its own.
export function createWrappedMutation(call: () => Promise<number>) {
  const [state] = createSignal<State>({ kind: "idle" });
  const run = async () => await call();
  return { state: () => state(), run };
}

// A method that reads `this`: it may reach `run` another way.
export function createThisMutation(call: () => Promise<number>) {
  const [state] = createSignal<State>({ kind: "idle" });
  const run = async () => await call();
  return {
    run,
    state(this: { run: () => Promise<number> }) {
      void this.run;
      return state();
    },
  };
}

// `bind` builds a new function the proof does not read.
export function createBoundMutation(call: () => Promise<number>) {
  const [state] = createSignal<State>({ kind: "idle" });
  const run = async () => await call();
  return { state: state.bind(null), run };
}

// The tuple is held before it is destructured: not the primitive's result.
export function createHeldMutation(call: () => Promise<number>) {
  const pair = createSignal<State>({ kind: "idle" });
  const [state] = pair;
  const run = async () => await call();
  return { state, run };
}

declare function opaqueSignal(): [Accessor<State>, (next: State) => void];

// An accessor from a call the proof cannot resolve.
export function createOpaqueMutation(call: () => Promise<number>) {
  const [state] = opaqueSignal();
  const run = async () => await call();
  return { state, run };
}

// A local function spelled `createSignal`: not the primitive.
export function createShadowedMutation(call: () => Promise<number>) {
  function createSignal<T>(value: T): [Accessor<T>] {
    const holder = {
      value,
      read(this: { value: T }) {
        return this.value;
      },
    };
    return [holder.read];
  }
  const [state] = createSignal<State>({ kind: "idle" });
  const run = async () => await call();
  return { state, run };
}
