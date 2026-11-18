import { render } from "@solidjs/web";
import { Ambient, ElementCall, ElementHeld, ForOf, ForOfHeld, ForOfHolder, HostRead, ThisGetter } from "./App";

render(() => <ElementCall />, document.body);
render(() => <ElementHeld />, document.body);
render(() => <ForOf />, document.body);
render(() => <ForOfHolder />, document.body);
render(() => <ForOfHeld />, document.body);
render(() => <ThisGetter />, document.body);
render(() => <HostRead />, document.body);
render(() => <Ambient />, document.body);
