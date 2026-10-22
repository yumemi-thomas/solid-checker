// SC2007 static-dynamic-async-source through solid-js@2.0.0-rc.17's
// dynamicComponent, which is dynamicCore (dist/solid.dev.js:1545): its static
// form throws "dynamic(): a static source must resolve synchronously" when the
// source returns a promise. The violation calls it from a timer; the control
// passes a synchronous source with the same option.
import { dynamicComponent } from "solid-js";

const Plain = () => <div />;

export function Violation() {
  setTimeout(() => {
    dynamicComponent(async () => Plain, { static: true });
  }, 0);
  return <div />;
}

export function Control() {
  setTimeout(() => {
    dynamicComponent(() => Plain, { static: true });
  }, 0);
  return <div />;
}
