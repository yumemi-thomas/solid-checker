import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

function Read() {
  const location = useLocation();
  const key: keyof Location = "search";
  let target: { -readonly [P in keyof Location]?: Location[P] } = {};
  target = location;
  delete target[key];
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
