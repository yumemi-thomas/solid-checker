import { action, flush } from "solid-js";
const act = action(function* () {
  (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { flush(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })();
  yield 1;
});
export function App() {
  setTimeout(() => { void act(); });
  return <i>app</i>;
}
