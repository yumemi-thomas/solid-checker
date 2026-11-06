import { createStore, flush } from "solid-js";
import { dynamic } from "@solidjs/web";

function Body() {
  const [s] = createStore({ a: { v: 1 } });
  flush(() => {
    const x = s.a;
    void x;
  });
  return <i />;
}

const D = dynamic(() => Body);
export const App = () => <D />;
