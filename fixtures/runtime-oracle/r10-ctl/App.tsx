import "./runtime";
declare function sink(x: unknown): void;
import { Show } from "solid-js";
import { useParams } from "@solidjs/router";
function helper(v: string) {
  sink(v);
}
function C() {
  const params = useParams();
  return <Show when={params.id}>{(id) => { helper(id()); return <span />; }}</Show>;
}

import { createRouter as makeOracleRouter, memoryHistory as oracleMemoryHistory } from "@solidjs/router";
const Router = makeOracleRouter({ routes: [{ path: "/:id/:a/:b", component: C }], history: oracleMemoryHistory("/1/2/3") });

import { useNavigate as oracleNavigate } from "@solidjs/router";
function OracleNavigation() {
  const go = oracleNavigate();
  return <button id="oracle-next" onClick={() => go("/4/5/6")}>navigate</button>;
}
export function App() { return <Router>{(props) => <><OracleNavigation />{props.children}</>}</Router>; }
