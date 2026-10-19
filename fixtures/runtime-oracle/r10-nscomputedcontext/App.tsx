import "./runtime";
declare function sink(x: unknown): void;
import { Show } from "solid-js";
import * as R from "@solidjs/router";
import { useParams } from "@solidjs/router";
const k = ["Router", "Context"].join("");
export const Ctx = (R as unknown as Record<string, unknown>)[k];
function helper(v: unknown) {
  sink(v);
}
function C() {
  const params = useParams();
  return <Show when={params.id}>{(v) => { helper(v()); return <span />; }}</Show>;
}

import { createRouter as makeOracleRouter, memoryHistory as oracleMemoryHistory } from "@solidjs/router";
const Router = makeOracleRouter({ routes: [{ path: "/:id/:a/:b", component: C }], history: oracleMemoryHistory("/1/2/3") });

import { useNavigate as oracleNavigate } from "@solidjs/router";
function OracleNavigation() {
  const go = oracleNavigate();
  return <button id="oracle-next" onClick={() => go("/4/5/6")}>navigate</button>;
}
export function App() { return <Router>{(props) => <><OracleNavigation />{props.children}</>}</Router>; }
