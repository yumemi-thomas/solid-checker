import { action } from "solid-js";
import "fz-r2-y2";
const act = action(function* () {});
const el = document.createElement("div");
export function C0() {
  el.addEventListener("fzready", () => {
    void act();
  });
  return null;
}
export function C1() {
  void Promise.resolve(1).then(function fzsync() {
    void act();
  });
  return null;
}
export function App() {
  return <C1 />;
}
