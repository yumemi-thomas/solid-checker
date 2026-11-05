import { render } from "@solidjs/web";
import { Destructures, Handlers } from "./App";

render(
  () => (
    <i>
      <Handlers />
      <Destructures />
    </i>
  ),
  document.body,
);
