import { render } from "@solidjs/web";
import { CallingParent, ConditionalParent, Grand, PlainParent, PropParent, ShowParent } from "./App";

render(() => <PlainParent />, document.body);
render(() => <Grand />, document.body);
render(() => <CallingParent />, document.body);
render(() => <ShowParent />, document.body);
render(() => <PropParent />, document.body);
render(() => <ConditionalParent />, document.body);
