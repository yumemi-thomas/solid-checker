import { createSignal, useContext } from "solid-js";
import { Router, RouterContext, useLocation } from "dep-router";
import type { Location } from "dep-router";

import { StaticWrap } from "dep-wrap";
function Read() {
  const location = useLocation();
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><StaticWrap><Read /></StaticWrap></Router>; }
