import { createSignal, onSettled } from "solid-js";
import { sync as stateSync, s as stateS } from "./state";

// The program-wide veto (ADR 0874, `handed-root-patch`): an unspelled-key write into a handed object (the router's `hashKey`) may replace any standard
// member, so a callback handed to a standard call (`then`) is code an impostor may run or never run: nothing places it.
export function hashKey(args: unknown) {
  return JSON.stringify(args, (_, val) => val && typeof val === "object"
    ? Object.keys(val).sort().reduce((result: any, key: string) => { result[key] = val[key]; return result; }, Object.create(null))
    : val);
}
declare function load(): Promise<void>;
declare const bus: { addListener(cb: () => void): () => void };

// `sync` is entered from a callback of a withheld `then` and from a listener nested in it. The walk that follows the paths asks the veto of each caller and
// answers Unknown; the write walk's region fold (ADR 0546) did not ask, placed both as a deferred callback and left the write clean (ADR 0991).
export function Settled() {
  const [s, setS] = createSignal(0);
  const sync = () => { setS(1); };
  onSettled(() => {
    void load().then(() => { bus.addListener(() => { sync(); }); sync(); });
  });
  return <p>{s()}</p>;
}

// An arrow alias of the call is the same caller.
export function Alias() {
  const [s, setS] = createSignal(0);
  const sync = () => { setS(1); };
  const run = () => sync();
  onSettled(() => {
    void load().then(() => { bus.addListener(() => { run(); }); run(); });
  });
  return <p>{s()}</p>;
}

// The callee is in another file than the withheld callers (it is exported: callers nothing sees make it uncertifiable in either build).
export function CrossFile() {
  onSettled(() => {
    void load().then(() => { bus.addListener(() => { stateSync(); }); stateSync(); });
  });
  return <p>{stateS()}</p>;
}
