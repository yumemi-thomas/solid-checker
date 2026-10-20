window.history.replaceState(null, "", "/1/2/3");
import "./runtime";
declare function sink(x: unknown): void;
import { Show } from "solid-js";
import { createRouter, useParams } from "@solidjs/router";
const cfg = { routes: [{ path: "/:id/:a/:b", component: C }] };
const Router = createRouter(cfg);
function helper(v: unknown) {
  sink(v);
}
function C() {
  const params = useParams();
  return <Show when={params.id}>{(v) => { helper(v()); return <span />; }}</Show>;
}

import { useNavigate as oracleNavigate } from "@solidjs/router";
function OracleNavigation() {
  const go = oracleNavigate();
  return <button id="oracle-next" onClick={() => go("/4/5/6")}>navigate</button>;
}
export function App() { return <Router>{(props) => <><OracleNavigation />{props.children}</>}</Router>; }
