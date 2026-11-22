import { render } from "@solidjs/web";
import { Apply, Body, Override, Rest } from "./App";

render(() => <Body />, document.body);
render(() => <Apply />, document.body);
render(() => <Override />, document.body);
render(() => <Rest />, document.body);
