import { render } from "@solidjs/web";
import { DirectBinding, IdentityWrapper, NeverRuns, UnknownWrapper } from "./App";

render(() => <NeverRuns />, document.body);
render(() => <UnknownWrapper />, document.body);
render(() => <IdentityWrapper />, document.body);
render(() => <DirectBinding />, document.body);
