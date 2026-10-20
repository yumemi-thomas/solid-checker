import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

function Read() {
  const location = useLocation();
  const key: keyof Location = "search";
  const target = location || {};
  delete (target as { -readonly [P in keyof Location]?: Location[P] })[key];
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
