import { render } from "@solidjs/web";
import { Open, Pages, Proved, Rendered } from "./App";

render(
  () => (
    <div>
      <Open />
      <Proved />
      <Rendered />
      <Pages />
    </div>
  ),
  document.body,
);
