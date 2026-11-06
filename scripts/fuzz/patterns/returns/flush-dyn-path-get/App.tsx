import { createStore, flush } from "solid-js";
import { dynamic } from "@solidjs/web";

function Body() {
  const [s] = createStore({ a: { v: 1 } });
  flush(() => {
    s.a.v = 7;
  });
  return <i />;
}

const D = dynamic(() => Body);
export const App = () => <D />;
