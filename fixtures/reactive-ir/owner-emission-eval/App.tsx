import { onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// B3 (the review of #221). A program that calls direct `eval` reaches any binding by name, so every layer of it is an open entry: `Evaled` is rendered by a tag, and the timer's
// `eval("Evaled()")` calls it with no owner (Chrome rc.21: NO_OWNER_CLEANUP). The entry walk asked `eval` after the two stops that closed a component and an owner-creating
// layer, so both stood; it now asks it before them. `Other` is no more reached by the eval than any other function of the program, and is open too.
function Evaled() {
  onCleanup(() => {});
  return <p>evaled</p>;
}
function Other() {
  onCleanup(() => {});
  return <p>other</p>;
}
function App() {
  setTimeout(() => {
    eval("Evaled()");
  }, 0);
  return (
    <p>
      <Evaled />
      <Other />
    </p>
  );
}
render(() => <App />, document.body);
