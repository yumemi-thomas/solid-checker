// ADR 0980, B6(e): the call is in a third module. Chrome: FLUSH_IN_ACTION; with this harness's entry (`main.tsx`) main already reports it uncertifiable; the entry-less fixture `flush-holder-put-crossfile` is its false clean.
import { flush } from "solid-js";
import { runStep } from "fz-step";
import { relay } from "./relay";
const o: any = {};
relay(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
