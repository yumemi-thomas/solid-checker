import { render } from "@solidjs/web";
import { DeepHelper, HelperCall, NamedHandler } from "./App";

render(() => <HelperCall />, document.body);
render(() => <DeepHelper />, document.body);
render(() => <NamedHandler />, document.body);
