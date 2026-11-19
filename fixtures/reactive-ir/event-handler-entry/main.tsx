import { render } from "@solidjs/web";
import { InCompute, Named, Resolve, Until } from "./App";

render(() => <Resolve />, document.body);
render(() => <Until />, document.body);
render(() => <Named />, document.body);
render(() => <InCompute />, document.body);

// ADR 1089 part E round 2: a timer handed a Promise executor's resolve function runs no code built from a string, so it is no
// hazard of this realm (the fuzz harness's own `pause` helper).
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));
void pause(0);
