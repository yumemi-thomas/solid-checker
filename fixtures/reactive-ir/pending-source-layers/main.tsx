import { render } from "@solidjs/web";
import { PropClosure, PropHelper, PropIife, PropKept, PropThrowFirst } from "./App";

const host = document.body;
render(() => <PropIife />, host);
render(() => <PropClosure />, host);
render(() => <PropHelper />, host);
render(() => <PropKept />, host);
render(() => <PropThrowFirst />, host);
