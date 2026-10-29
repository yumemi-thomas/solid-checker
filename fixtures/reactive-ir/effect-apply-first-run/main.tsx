import { render } from "@solidjs/web";
import { Proven, RenderedSecond } from "./App";

const host = document.body;
render(() => <Proven />, host);
render(() => <RenderedSecond />, host);
