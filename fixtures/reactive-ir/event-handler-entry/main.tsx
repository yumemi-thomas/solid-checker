import { render } from "@solidjs/web";
import { InCompute, Named, Resolve, Until } from "./App";

render(() => <Resolve />, document.body);
render(() => <Until />, document.body);
render(() => <Named />, document.body);
render(() => <InCompute />, document.body);
