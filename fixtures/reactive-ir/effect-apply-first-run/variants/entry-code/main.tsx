import { render } from "@solidjs/web";
import { C, prepare } from "./App";

prepare();
render(() => <C />, document.body);
