import { lazy, Loading } from "solid-js";
import { clientOnly, render } from "@solidjs/web";

const Client = clientOnly(() => import("./ClientBody"));
const Picked = clientOnly(() => import("./Picked").then((m) => ({ default: m.Named })));
const Lazy = lazy(() => import("./LazyBody"));

function App() {
  return (
    <Loading fallback={<i />}>
      <Client />
      <Picked />
      <Lazy />
    </Loading>
  );
}

render(() => <App />, document.body);
