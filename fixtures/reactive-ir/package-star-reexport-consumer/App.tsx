import { isReady, isReadyOpen } from "./star";
import { isReadyOpen as chained } from "./chain";
import { ns } from "./namespace";
import { isReadyOpen as local } from "./shadow";

export function Status() {
  return <div>{isReady() && local() ? "ready" : "waiting"}</div>;
}
export function StatusOpen() {
  return <div>{isReadyOpen() && chained() && ns.isReadyOpen() ? "ready" : "waiting"}</div>;
}
