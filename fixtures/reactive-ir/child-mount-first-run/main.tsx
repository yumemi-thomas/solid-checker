import { render } from "@solidjs/web";
import { CallingParent, ConditionalParent, ForEmptyParent, ForItemParent, ForParent, Grand, PlainParent, PropParent, ShowBindingParent, ShowParent } from "./App";

render(() => <PlainParent />, document.body);
render(() => <Grand />, document.body);
render(() => <CallingParent />, document.body);
render(() => <ShowParent />, document.body);
render(() => <PropParent />, document.body);
render(() => <ConditionalParent />, document.body);
render(() => <ForParent />, document.body);
render(() => <ShowBindingParent />, document.body);
render(() => <ForEmptyParent />, document.body);
render(() => <ForItemParent />, document.body);
