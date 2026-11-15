import { createSignal } from "solid-js";
import { Router, useLocation } from "dep-shell";

function Read() {
  const location = useLocation();
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
