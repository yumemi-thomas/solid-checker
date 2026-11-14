// ADR 0980, B6(e): the function is re-exported by name from another module. Chrome: FLUSH_IN_ACTION; with this harness's entry (`main.tsx`) main already reports it uncertifiable; the entry-less fixture `flush-holder-put-crossfile` is its false clean.
import { flush } from "solid-js";
import { runStep } from "fz-step";
import { put } from "./barrel";
const o: any = {};
put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
