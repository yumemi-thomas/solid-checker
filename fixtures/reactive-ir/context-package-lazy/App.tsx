import { lazy } from "solid-js";

const FromPackage = lazy(() => import("dep-ui-context").then((m) => ({ default: m.Panel })));
const FromProject = lazy(() => import("./Local"));

export function Package() {
  return <FromPackage>x</FromPackage>;
}
export function Project() {
  return <FromProject>x</FromProject>;
}
