import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

function Read() {
  const location = useLocation();
  const key: keyof Location = "search";
  let mutable: { [key: string]: string } = {};
  mutable = {};
  delete mutable[key];
  const logical = mutable || {};
  const conditional: { [key: string]: string } = Math.random() ? logical : {};
  delete conditional[key];
  const { target } = { target: conditional };
  const [arrayTarget] = [target];
  delete arrayTarget[key];
  function strip({ target }: { target: { [key: string]: string } }) { delete target[key]; }
  strip({ target: {} });
  strip({ target: {} });
  function stripArray([target]: [{ [key: string]: string }]) { delete target[key]; }
  stripArray([{}]);
  function stripThis(this: { [key: string]: string }) { delete this[key]; }
  stripThis.call({});
  stripThis.apply({}, []);
  const holder = { strip() { delete (this as { [key: string]: unknown })[key]; } };
  holder.strip();
  const arrow = (() => {}) as (() => void) & { [key: string]: unknown };
  const expression = (function () {}) as (() => void) & { [key: string]: unknown };
  delete arrow[key];
  delete expression[key];
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
