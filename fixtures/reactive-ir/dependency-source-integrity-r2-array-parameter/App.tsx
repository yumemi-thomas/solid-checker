import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

function Read() {
  const location = useLocation();
  const key: keyof Location = "search";
  function strip([target]: [Location]) {
    delete (target as { -readonly [P in keyof Location]?: Location[P] })[key];
  }
  strip([location]);
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
