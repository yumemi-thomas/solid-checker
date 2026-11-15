import { createSignal, onSettled } from "solid-js";

// The same program without the veto (no write of an unspelled key into a handed object): `then` is the standard one, its callback runs later with no
// owner, and the listener nested in it is a deferred callback too. The walk and the fold place the write the same way: no finding in either build (ADR 0991).
declare function load(): Promise<void>;
declare const bus: { addListener(cb: () => void): () => void };

export function Settled() {
  const [s, setS] = createSignal(0);
  const sync = () => { setS(1); };
  onSettled(() => {
    void load().then(() => { bus.addListener(() => { sync(); }); sync(); });
  });
  return <p>{s()}</p>;
}

export function Alias() {
  const [s, setS] = createSignal(0);
  const sync = () => { setS(1); };
  const run = () => sync();
  onSettled(() => {
    void load().then(() => { bus.addListener(() => { run(); }); run(); });
  });
  return <p>{s()}</p>;
}
