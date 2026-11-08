import { render } from "@solidjs/web";
import "./gateway";
import { BodyRead } from "./body-read";
import { Tracked } from "./tracked";
import { Escaped } from "./escaped";
render(() => <><BodyRead /><Tracked /><Escaped /></>, document.body);
