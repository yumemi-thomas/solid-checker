import { render } from "@solidjs/web";
import { BodyHelper, BodyRead, Shown } from "./App";

render(() => <Shown />, document.body);
render(() => <BodyHelper />, document.body);
render(() => <BodyRead />, document.body);
