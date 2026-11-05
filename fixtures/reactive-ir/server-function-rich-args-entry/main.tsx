// ADR 0707 (SSR audit D10): the client entry calls enableRichArguments() at
// module scope before render, so the codec is installed before any call:
// SC7007 stays silent in this project.
import { enableRichArguments } from "@solidjs/web/server-functions/rich-args";
import { render } from "@solidjs/web";
import { Toolbar } from "./App";

enableRichArguments();
render(() => <Toolbar />, document.body);
