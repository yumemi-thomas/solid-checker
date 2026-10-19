import "./runtime";
declare function sink(x: unknown): void;
import { Show } from "solid-js";
import { createRouter as cr, useParams } from "@solidjs/router";
import * as R from "@solidjs/router";
const k = ["params", "Wrapper"].join("");
const make = R.createRouter;
const A = cr({ routes: [{ path: "/:id/:a/:b", component: C }] });
const B = make({ routes: [{ path: "/:id/:a/:b", component: C }], history: { ...R.memoryHistory("/1/2/3"), utils: { [k]: () => ({ id: {} }) } } as R.RouterHistory });
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
