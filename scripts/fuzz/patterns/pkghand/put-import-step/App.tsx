// ADR 0980, B6(e): a named import under a closed program boundary. Chrome: FLUSH_IN_ACTION; with this harness's entry (`main.tsx`) main already reports it uncertifiable; the entry-less fixture `flush-holder-put-crossfile` is its false clean.
import { flush } from "solid-js";
import { runStep } from "fz-step";
import { put } from "./put";
const o: any = {};
put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
