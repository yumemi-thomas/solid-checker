// SC2007 static-dynamic-async-source. The dev build's static dynamic throws
// "dynamic(): a static source must resolve synchronously" when its source
// returns a promise. The violation calls dynamic() with an async source and
// { static: true } from a timer; the control passes a synchronous source with
// the same option.
import { dynamic } from "@solidjs/web";

const Plain = () => <div />;

export function Violation() {
  setTimeout(() => {
    dynamic(async () => Plain, { static: true });
  }, 0);
  return <div />;
}

export function Control() {
  setTimeout(() => {
    dynamic(() => Plain, { static: true });
  }, 0);
  return <div />;
}
