import { render } from "@solidjs/web";
import { ApplyCall, Closure, Direct, Iife, RenderCompute } from "./App";

const host = document.body;
render(() => <Direct />, host);
render(() => <Closure />, host);
render(() => <Iife />, host);
render(() => <RenderCompute />, host);
render(() => <ApplyCall />, host);
