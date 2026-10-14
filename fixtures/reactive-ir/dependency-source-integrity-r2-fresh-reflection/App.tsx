import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

function Read() {
  const location = useLocation();
  const key: keyof Location = "search";
  const unrelated = {};
  Object.defineProperties(unrelated, { [key]: { value: "inert" } });
  Reflect.deleteProperty(unrelated, key);
  Reflect.set(unrelated, key, "inert");
  Reflect.defineProperty(unrelated, key, { value: "inert" });
  Object.assign(unrelated, Math.random() ? {} : {});
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
