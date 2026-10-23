import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);

// No violation: both entries of the nested `Writer` run from the handler (no
// rc.17 diagnostic); the enclosing component proves nothing about them. The
// `map` entry is not followed, so the write is uncertifiable.
export function NestedHost() {
  function Writer() {
    setCount(1);
    return <b />;
  }
  return <button onClick={() => { Writer(); [1].map(Writer); }} />;
}
