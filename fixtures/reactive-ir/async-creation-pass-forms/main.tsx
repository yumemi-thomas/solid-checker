import { render } from "@solidjs/web";
import { FirstParent, LaterParent } from "./App";

const host = document.body;
render(() => <FirstParent />, host);
render(() => <LaterParent />, host);
