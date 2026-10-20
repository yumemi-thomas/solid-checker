import "./mutate";
import { api } from "./api";

// `api.read` names the literal's method, but another module assigns the
// member: the receiver census withholds the call's target (ADR 0299), and
// the call is a dispatch obligation, not a call that reads nothing.
export function DirectRead() {
  const value = api.read();
  return <main>{value}</main>;
}
